//! `vantyx ai` command implementation.
//!
//! The CLI is the supported AI surface until modules, async, and a standard
//! library exist. It never downloads models or reads secrets from project
//! files.

use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use vantyx::ai::{
    self, AiConfig, Budget, ChatRequest, ChatResponse, DEFAULT_HASH_DIMENSIONS, EchoProvider,
    EmbeddingRequest, HashEmbeddingProvider, HttpTransport, Message, MetricEvent, MetricsSink,
    MockProvider, OpenAiProvider, Provider, ProviderCapabilities,
};

const AI_USAGE: &str = "\
Vantyx AI host tools (offline by default)

Usage:
  vantyx ai providers              List providers and capabilities
  vantyx ai doctor [options]       Check provider configuration or reachability
  vantyx ai models [options]       List models from an OpenAI-compatible endpoint
  vantyx ai chat [options] [prompt] Chat with a provider
  vantyx ai extract [options] [prompt]
                                   Validate JSON output against a schema
  vantyx ai embed [options] [text...]
                                   Create embeddings; mock/hash is offline and
                                   deterministic, openai uses /v1/embeddings
  vantyx ai index --root <dir> --out <file> [options]
                                   Chunk documents, embed them, and save a
                                   local retrieval index with line citations
  vantyx ai search --index <file> --query <text> [options]
                                   Search a saved index (lexical, semantic, or
                                   hybrid) and print cited chunks
  vantyx ai eval --index <file> --fixture <file> [options]
                                   Score retrieval against labelled queries
  vantyx ai tokens [text]          Estimate tokens (not a model tokenizer)

Options:
  --provider <name>       mock, echo, hash (embed only), or openai
                          (default: env or mock)
  --endpoint <url>        OpenAI-compatible base URL
  --system <text>         System instruction
  --message <text>        User message; may be repeated
  --api-key-env <name>    Environment variable holding the API key
  --allow-network         Allow remote HTTPS (loopback HTTP is local)
  --timeout-ms <n>        Request timeout in milliseconds
  --max-tokens <n>        Completion token limit
  --temperature <n>       Sampling temperature, 0 to 2
  --dimensions <n>        Embedding dimensions, 1 to 4096
  --metrics <path>        Append redacted JSONL metrics (no prompts)
  --schema <path|json>    JSON Schema for `extract` (file path or inline object)
  --repair-attempts <n>   Repair attempts after invalid output (0-3; default 1)
  --response <text>       Offline mock response for `extract`; may be repeated
  --root <dir>            Document root for `index`
  --out <file>            Output index path for `index`
  --index <file>          Index path for `search` and `eval`
  --query <text>          Search query for `search` (or positional text)
  --source-prefix <path>  Only search chunks whose relative source starts with this path
  --fixture <file>        Evaluation fixture for `eval`
  --mode <name>           lexical, semantic, or hybrid (default: inferred)
  --top-k <n>             Results or evaluation depth, 1 to 100
  --diverse               Rerank search results with deterministic MMR
                          diversity (relevance weight 0.7)
  --chunk-chars <n>       Maximum characters per chunk (default 900)
  --overlap-chars <n>     Chunk overlap in characters (default 120)
  --ext <ext>             Indexed extension; repeat or comma-separate
  --incremental           Update an existing semantic index, reusing vectors
                          for unchanged chunks and pruning deleted chunks
  --stream                Print OpenAI-compatible text deltas as they arrive
                          (not compatible with --json)
  --json                  Print the response as JSON
  -                       Read the prompt from standard input
  -h, --help              Show this help

Environment:
  VANTYX_AI_PROVIDER, VANTYX_AI_MODEL, VANTYX_AI_ENDPOINT,
  VANTYX_AI_API_KEY_ENV, VANTYX_AI_ALLOW_NETWORK,
  VANTYX_AI_TIMEOUT_MS, VANTYX_AI_MAX_RESPONSE_BYTES

No model is bundled or downloaded. Local endpoints need Ollama, llama.cpp,
LM Studio, or another compatible server already running.
";

#[derive(Default)]
struct Options {
    provider: Option<String>,
    model: Option<String>,
    endpoint: Option<String>,
    api_key_env: Option<String>,
    allow_network: bool,
    timeout_ms: Option<u64>,
    max_tokens: Option<u32>,
    temperature: Option<f64>,
    dimensions: Option<usize>,
    system: Option<String>,
    messages: Vec<String>,
    positional: Vec<String>,
    metrics: Option<PathBuf>,
    schema: Option<String>,
    repair_attempts: Option<usize>,
    responses: Vec<String>,
    root: Option<PathBuf>,
    out: Option<PathBuf>,
    index: Option<PathBuf>,
    query: Option<String>,
    source_prefix: Option<String>,
    fixture: Option<PathBuf>,
    mode: Option<String>,
    top_k: Option<usize>,
    chunk_chars: Option<usize>,
    overlap_chars: Option<usize>,
    extensions: Vec<String>,
    incremental: bool,
    diverse: bool,
    stream: bool,
    json: bool,
    prompt_stdin: bool,
    help: bool,
}

pub fn run(args: &[String]) -> i32 {
    let Some(command) = args.first().map(String::as_str) else {
        eprint!("{AI_USAGE}");
        return 64;
    };
    let rest = &args[1..];
    match command {
        "help" | "--help" | "-h" => {
            print!("{AI_USAGE}");
            0
        }
        "providers" => providers(rest),
        "doctor" => doctor(rest),
        "models" => models(rest),
        "chat" => chat(rest),
        "extract" => extract_command(rest),
        "embed" => embed(rest),
        "index" => rag_index(rest),
        "search" => rag_search(rest),
        "eval" => rag_eval(rest),
        "tokens" => tokens(rest),
        other => {
            eprintln!("error: unknown `vantyx ai` command `{other}`");
            eprintln!();
            eprint!("{AI_USAGE}");
            64
        }
    }
}

fn providers(args: &[String]) -> i32 {
    if !args.is_empty() {
        eprintln!("error: `vantyx ai providers` takes no arguments");
        return 64;
    }
    let config = AiConfig::from_env();
    println!("mock    offline deterministic chat provider; no network or model");
    println!(
        "        capabilities: {}",
        capabilities(&ProviderCapabilities::mock())
    );
    println!("hash    offline deterministic feature-hash embeddings; not a semantic model");
    println!(
        "        capabilities: {}",
        capabilities(&ProviderCapabilities::hash_embedding())
    );
    println!("echo    offline provider that returns the user text");
    println!(
        "        capabilities: {}",
        capabilities(&ProviderCapabilities::default())
    );
    println!("openai  OpenAI-compatible HTTP endpoint");
    println!("        endpoint: {}", config.endpoint);
    println!(
        "        api key: {}",
        if config.api_key_present() {
            format!("set via {}", config.api_key_env)
        } else {
            format!(
                "unset (optional for local endpoints; set {})",
                config.api_key_env
            )
        }
    );
    println!(
        "        network: {}",
        if config.allow_network {
            "remote HTTPS enabled"
        } else {
            "loopback only (pass --allow-network for remote HTTPS)"
        }
    );
    println!(
        "        capabilities: {}",
        capabilities(&ProviderCapabilities::openai_compatible())
    );
    0
}

fn doctor(args: &[String]) -> i32 {
    let options = match parse_options(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("error: {message}");
            return 64;
        }
    };
    if options.help {
        print!("{AI_USAGE}");
        return 0;
    }
    let config = resolve_config(&options);
    let provider = options
        .provider
        .clone()
        .unwrap_or_else(|| config.provider.clone());
    let model = options
        .model
        .clone()
        .unwrap_or_else(|| config.model.clone());
    match provider.as_str() {
        "mock" | "echo" => {
            println!("provider: {provider}");
            println!("status: ok (offline; no network or model required)");
            println!("model: {model}");
            let caps = if provider == "mock" {
                ProviderCapabilities::mock()
            } else {
                ProviderCapabilities::default()
            };
            println!("capabilities: {}", capabilities(&caps));
            0
        }
        "hash" => {
            let dimensions = options.dimensions.unwrap_or(DEFAULT_HASH_DIMENSIONS);
            println!("provider: hash");
            println!("status: ok (offline; deterministic feature hashing, not a semantic model)");
            println!("dimensions: {dimensions}");
            println!(
                "capabilities: {}",
                capabilities(&ProviderCapabilities::hash_embedding())
            );
            0
        }
        "openai" => {
            let endpoint = config.endpoint.clone();
            let openai = match build_openai(&config, &model) {
                Ok(openai) => openai,
                Err(error) => return report_error(&error),
            };
            println!("provider: openai");
            println!("endpoint: {endpoint}");
            println!(
                "network: {}",
                if config.allow_network {
                    "remote HTTPS enabled"
                } else {
                    "loopback only"
                }
            );
            match openai.models() {
                Ok(models) => {
                    println!("status: ok");
                    println!("models: {}", models.len());
                    for model in models.iter().take(10) {
                        println!("  {model}");
                    }
                    if models.len() > 10 {
                        println!("  ... {} more", models.len() - 10);
                    }
                    0
                }
                Err(error) => {
                    eprintln!("status: failed");
                    report_error(&error)
                }
            }
        }
        other => {
            eprintln!("error: unknown provider `{other}`; expected mock, echo, hash, or openai");
            64
        }
    }
}

fn models(args: &[String]) -> i32 {
    let options = match parse_options(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("error: {message}");
            return 64;
        }
    };
    if options.help {
        print!("{AI_USAGE}");
        return 0;
    }
    let config = resolve_config(&options);
    let provider = options
        .provider
        .clone()
        .unwrap_or_else(|| config.provider.clone());
    let model = options
        .model
        .clone()
        .unwrap_or_else(|| config.model.clone());
    match provider.as_str() {
        "openai" => {}
        "mock" | "echo" | "hash" => {
            eprintln!("error: provider `{provider}` has no model list; use --provider openai");
            return 64;
        }
        other => {
            eprintln!("error: unknown provider `{other}`; expected mock, echo, hash, or openai");
            return 64;
        }
    }
    let openai = match build_openai(&config, &model) {
        Ok(openai) => openai,
        Err(error) => return report_error(&error),
    };
    match openai.models() {
        Ok(models) => {
            for model in models {
                println!("{model}");
            }
            0
        }
        Err(error) => report_error(&error),
    }
}

fn chat(args: &[String]) -> i32 {
    let options = match parse_options(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("error: {message}");
            return 64;
        }
    };
    if options.help {
        print!("{AI_USAGE}");
        return 0;
    }
    let config = resolve_config(&options);
    let provider_name = options
        .provider
        .clone()
        .unwrap_or_else(|| config.provider.clone());
    let model = options
        .model
        .clone()
        .unwrap_or_else(|| config.model.clone());
    let messages = match build_messages(&options) {
        Ok(messages) => messages,
        Err(code) => return code,
    };
    if options.stream && options.json {
        eprintln!("error: `--stream` and `--json` cannot be combined");
        eprintln!("help: use `--stream` for live text, or `--json` for one final object");
        return 64;
    }
    if options.stream && provider_name != "openai" {
        eprintln!(
            "error: provider `{provider_name}` does not support streaming; use --provider openai"
        );
        return 64;
    }

    let provider = match build_provider(&config, &provider_name, &model) {
        Ok(provider) => provider,
        Err(error) => return report_error(&error),
    };
    let mut request = ChatRequest::new(model.clone(), messages);
    request.temperature = options.temperature;
    request.max_tokens = options.max_tokens;
    request.timeout_ms = options.timeout_ms;
    if let Err(error) = request.validate() {
        return report_error(&error);
    }

    let mut budget = Budget::default();
    let started = Instant::now();
    let metrics = options
        .metrics
        .as_ref()
        .map_or_else(MetricsSink::disabled, MetricsSink::file);
    let mut wrote_delta = false;
    let result = if options.stream {
        provider.chat_stream(&request, &mut budget, &mut |delta| {
            print!("{delta}");
            let _ = io::stdout().flush();
            wrote_delta = true;
        })
    } else {
        provider.chat(&request, &mut budget)
    };
    match result {
        Ok(response) => {
            if options.stream {
                if wrote_delta {
                    println!();
                } else {
                    println!("(empty response)");
                }
            } else if options.json {
                println!("{}", response.to_json());
            } else {
                let text = response.message.text_content();
                if !text.is_empty() {
                    println!("{text}");
                }
                for call in &response.message.tool_calls {
                    println!("tool_call {} {}", call.name, call.arguments);
                }
                if text.is_empty() && response.message.tool_calls.is_empty() {
                    println!("(empty response)");
                }
            }
            let event = MetricEvent::new(provider.id(), &model, "chat")
                .success(started.elapsed().as_millis() as u64)
                .usage(
                    response.usage.prompt_tokens,
                    response.usage.completion_tokens,
                    response.usage.total_tokens,
                )
                .finish_reason(response.finish_reason.as_str());
            if let Err(error) = metrics.record(&event) {
                eprintln!("warning: {error}");
            }
            0
        }
        Err(error) => {
            let event = MetricEvent::new(provider.id(), &model, "chat")
                .failure(started.elapsed().as_millis() as u64, error.code.clone());
            if let Err(metrics_error) = metrics.record(&event) {
                eprintln!("warning: {metrics_error}");
            }
            report_error(&error)
        }
    }
}

fn extract_command(args: &[String]) -> i32 {
    let options = match parse_options(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("error: {message}");
            return 64;
        }
    };
    if options.help {
        print!("{AI_USAGE}");
        return 0;
    }
    let Some(schema_argument) = options.schema.as_deref() else {
        eprintln!("error: `vantyx ai extract` requires --schema <path|json>");
        eprintln!("help: pass a JSON Schema file or an inline JSON object");
        return 64;
    };
    let schema = match load_schema(schema_argument) {
        Ok(schema) => schema,
        Err(message) => {
            eprintln!("error: {message}");
            return 64;
        }
    };
    let config = resolve_config(&options);
    let provider_name = options
        .provider
        .clone()
        .unwrap_or_else(|| config.provider.clone());
    let model = options
        .model
        .clone()
        .unwrap_or_else(|| config.model.clone());
    let messages = match build_messages(&options) {
        Ok(messages) => messages,
        Err(code) => return code,
    };
    let provider: Box<dyn Provider> = match provider_name.as_str() {
        "mock" => {
            if options.responses.is_empty() {
                eprintln!("error: the mock provider needs at least one --response for extraction");
                eprintln!("help: use --provider openai for a real endpoint, or pass --response");
                return 64;
            }
            let responses = options
                .responses
                .iter()
                .map(|text| Ok(ChatResponse::text(&model, text.clone())))
                .collect();
            Box::new(MockProvider::new("mock", responses))
        }
        "echo" => Box::new(EchoProvider::new()),
        "openai" => match build_openai(&config, &model) {
            Ok(openai) => Box::new(openai),
            Err(error) => return report_error(&error),
        },
        other => {
            eprintln!(
                "error: unknown provider `{other}`; expected mock, echo, or openai (`hash` is embed-only)"
            );
            return 64;
        }
    };

    let mut request = ChatRequest::new(model.clone(), messages);
    request.temperature = options.temperature;
    request.max_tokens = options.max_tokens;
    request.timeout_ms = options.timeout_ms;
    let repair_attempts = options.repair_attempts.unwrap_or(1);
    let mut budget = Budget::default();
    let started = Instant::now();
    let metrics = options
        .metrics
        .as_ref()
        .map_or_else(MetricsSink::disabled, MetricsSink::file);
    match ai::extract(
        provider.as_ref(),
        request,
        &schema,
        &mut budget,
        repair_attempts,
    ) {
        Ok(result) => {
            if options.json {
                println!("{}", result.to_json());
            } else {
                println!("{}", result.value.to_compact_string());
            }
            let event = MetricEvent::new(provider.id(), &model, "extract")
                .success(started.elapsed().as_millis() as u64)
                .usage(
                    result.usage.prompt_tokens,
                    result.usage.completion_tokens,
                    result.usage.total_tokens,
                )
                .finish_reason(result.finish_reason.as_str());
            if let Err(error) = metrics.record(&event) {
                eprintln!("warning: {error}");
            }
            0
        }
        Err(error) => {
            let event = MetricEvent::new(provider.id(), &model, "extract")
                .failure(started.elapsed().as_millis() as u64, error.code.clone());
            if let Err(metrics_error) = metrics.record(&event) {
                eprintln!("warning: {metrics_error}");
            }
            report_error(&error)
        }
    }
}

fn embed(args: &[String]) -> i32 {
    let options = match parse_options(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("error: {message}");
            return 64;
        }
    };
    if options.help {
        print!("{AI_USAGE}");
        return 0;
    }
    if options.system.is_some()
        || !options.messages.is_empty()
        || options.schema.is_some()
        || options.repair_attempts.is_some()
        || !options.responses.is_empty()
        || options.max_tokens.is_some()
        || options.temperature.is_some()
    {
        eprintln!("error: `vantyx ai embed` accepts embedding options only");
        eprintln!("help: use positional text, `-`, --dimensions, --model, --endpoint, and --json");
        return 64;
    }
    let inputs = match embedding_inputs(&options) {
        Ok(inputs) => inputs,
        Err(code) => return code,
    };
    let config = resolve_config(&options);
    let provider_name = options
        .provider
        .clone()
        .unwrap_or_else(|| config.provider.clone());
    let model = options
        .model
        .clone()
        .unwrap_or_else(|| config.model.clone());
    let dimensions = options.dimensions;
    let provider: Box<dyn Provider> = match provider_name.as_str() {
        "mock" | "hash" => {
            let dimensions = dimensions.unwrap_or(DEFAULT_HASH_DIMENSIONS);
            match HashEmbeddingProvider::new(dimensions) {
                Ok(provider) => Box::new(provider),
                Err(error) => return report_error(&error),
            }
        }
        "openai" => match build_openai(&config, &model) {
            Ok(openai) => Box::new(openai),
            Err(error) => return report_error(&error),
        },
        "echo" => {
            eprintln!("error: provider `echo` has no embeddings; use mock/hash or openai");
            return 64;
        }
        other => {
            eprintln!("error: unknown provider `{other}`; expected mock, echo, hash, or openai");
            return 64;
        }
    };

    let mut request = EmbeddingRequest::new(model.clone(), inputs.clone());
    request.dimensions = dimensions;
    request.timeout_ms = options.timeout_ms;
    let mut budget = Budget::default();
    let started = Instant::now();
    let metrics = options
        .metrics
        .as_ref()
        .map_or_else(MetricsSink::disabled, MetricsSink::file);
    match provider.embed(&request, &mut budget) {
        Ok(response) => {
            if options.json {
                let mut entries = match response.to_json() {
                    ai::Json::Object(entries) => entries,
                    _ => unreachable!("embedding responses serialize as JSON objects"),
                };
                entries.insert(0, ("provider".into(), ai::Json::from(provider.id())));
                entries.push((
                    "input_count".into(),
                    ai::Json::from(u64::try_from(inputs.len()).unwrap_or(u64::MAX)),
                ));
                println!("{}", ai::Json::Object(entries));
            } else {
                println!("provider: {}", provider.id());
                println!("model: {}", response.model);
                println!("dimensions: {}", response.dimensions().unwrap_or(0));
                println!("inputs: {}", inputs.len());
                println!(
                    "usage: prompt_tokens={} total_tokens={}",
                    response.usage.prompt_tokens, response.usage.total_tokens
                );
                let mut previewed = false;
                for (index, vector) in response.vectors.iter().enumerate() {
                    let preview = vector
                        .values()
                        .iter()
                        .take(8)
                        .map(|value| format!("{value:.6}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let suffix = if vector.len() > 8 {
                        previewed = true;
                        ", ..."
                    } else {
                        ""
                    };
                    println!("[{index}] [{preview}{suffix}]");
                }
                if previewed {
                    println!(
                        "note: human output previews 8 components; use --json for full vectors"
                    );
                }
            }
            let event = MetricEvent::new(provider.id(), &model, "embed")
                .success(started.elapsed().as_millis() as u64)
                .usage(response.usage.prompt_tokens, 0, response.usage.total_tokens);
            if let Err(error) = metrics.record(&event) {
                eprintln!("warning: {error}");
            }
            0
        }
        Err(error) => {
            let event = MetricEvent::new(provider.id(), &model, "embed")
                .failure(started.elapsed().as_millis() as u64, error.code.clone());
            if let Err(metrics_error) = metrics.record(&event) {
                eprintln!("warning: {metrics_error}");
            }
            report_error(&error)
        }
    }
}

fn rag_index(args: &[String]) -> i32 {
    let options = match parse_options(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("error: {message}");
            return 64;
        }
    };
    if options.help {
        print!("{AI_USAGE}");
        return 0;
    }
    if !options.positional.is_empty() {
        eprintln!("error: `vantyx ai index` takes flags only; use --root and --out");
        return 64;
    }
    if options.metrics.is_some() {
        eprintln!("error: --metrics is not supported by `vantyx ai index` yet");
        return 64;
    }
    if options.source_prefix.is_some() {
        eprintln!("error: --source-prefix is only supported by `vantyx ai search`");
        return 64;
    }
    if options.diverse {
        eprintln!("error: --diverse is only supported by `vantyx ai search`");
        return 64;
    }
    let Some(root) = options.root.as_ref() else {
        eprintln!("error: `vantyx ai index` requires --root <dir>");
        return 64;
    };
    let Some(out) = options.out.as_ref() else {
        eprintln!("error: `vantyx ai index` requires --out <file>");
        return 64;
    };
    let build_mode = match options.mode.as_deref() {
        None | Some("semantic") => ai::SearchMode::Semantic,
        Some("lexical") => ai::SearchMode::Lexical,
        Some("hybrid") => {
            eprintln!("error: build an index with --mode semantic or --mode lexical");
            eprintln!("help: hybrid is a search mode; it needs stored vectors");
            return 64;
        }
        Some(other) => {
            eprintln!("error: unknown index mode `{other}`; expected semantic or lexical");
            return 64;
        }
    };
    let chunk_options = match ai::ChunkOptions::new(
        options
            .chunk_chars
            .unwrap_or(ai::rag::DEFAULT_CHUNK_CHARS),
        options
            .overlap_chars
            .unwrap_or(ai::rag::DEFAULT_OVERLAP_CHARS),
    ) {
        Ok(chunk_options) => chunk_options,
        Err(error) => return report_error(&error),
    };
    let mut ingest = ai::IngestOptions::default();
    if !options.extensions.is_empty() {
        ingest.extensions = options.extensions.clone();
    }
    let documents = match ai::collect_documents(root, &ingest) {
        Ok(documents) => documents,
        Err(error) => return report_error(&error),
    };
    let mut chunks = Vec::new();
    for document in &documents {
        match ai::chunk_document(&document.source, &document.text, &chunk_options) {
            Ok(mut document_chunks) => chunks.append(&mut document_chunks),
            Err(error) => return report_error(&error),
        }
    }

    let config = resolve_config(&options);
    let provider_name = options
        .provider
        .clone()
        .unwrap_or_else(|| config.provider.clone());
    let dimensions = options.dimensions;
    let hash_dimensions = dimensions.unwrap_or(DEFAULT_HASH_DIMENSIONS);
    let model = options.model.clone().unwrap_or_else(|| {
        if provider_name == "hash" || provider_name == "mock" {
            format!("hash-embedding-{hash_dimensions}")
        } else {
            config.model.clone()
        }
    });

    if build_mode == ai::SearchMode::Lexical {
        if options.incremental {
            eprintln!("error: --incremental requires a semantic index with stored vectors");
            eprintln!(
                "help: rebuild with --mode semantic, or omit --incremental for a lexical rebuild"
            );
            return 64;
        }
        let index = match ai::RagIndex::lexical("lexical", "none", chunks) {
            Ok(index) => index,
            Err(error) => return report_error(&error),
        };
        if let Err(error) = index.save(out) {
            return report_error(&error);
        }
        let bytes = std::fs::metadata(out).map(|metadata| metadata.len()).unwrap_or(0);
        if options.json {
            println!(
                "{}",
                ai::Json::object([
                    ("index", ai::Json::from(out.to_string_lossy().to_string())),
                    ("mode", ai::Json::from("lexical")),
                    ("provider", ai::Json::from("lexical")),
                    ("model", ai::Json::from("none")),
                    ("dimensions", ai::Json::from(0_u64)),
                    (
                        "documents",
                        ai::Json::from(u64::try_from(documents.len()).unwrap_or(u64::MAX)),
                    ),
                    (
                        "chunks",
                        ai::Json::from(u64::try_from(index.len()).unwrap_or(u64::MAX)),
                    ),
                    ("vectors", ai::Json::from(0_u64)),
                    ("bytes", ai::Json::from(bytes)),
                ])
            );
        } else {
            println!("index: {}", out.display());
            println!("mode: lexical (BM25 only; no embedding model called)");
            println!("documents: {}", documents.len());
            println!("chunks: {}", index.len());
            println!("bytes: {bytes}");
            println!("note: lexical indexes cannot serve semantic or hybrid search");
        }
        return 0;
    }

    if options.incremental {
        if !out.is_file() {
            eprintln!(
                "error: --incremental requires an existing index at `{}`",
                out.display()
            );
            eprintln!("help: build the index once without --incremental, then update it");
            return 64;
        }
        let previous = match ai::RagIndex::load(out) {
            Ok(index) => index,
            Err(error) => return report_error(&error),
        };
        if previous.dimensions() == 0 {
            eprintln!("error: --incremental requires a semantic index with stored vectors");
            eprintln!(
                "help: rebuild with --mode semantic, or omit --incremental for a lexical rebuild"
            );
            return 64;
        }
        if let Some(requested) = options.provider.as_deref()
            && requested != previous.provider()
        {
            eprintln!(
                "error: --incremental cannot change the provider from `{}` to `{requested}`",
                previous.provider()
            );
            eprintln!("help: rebuild without --incremental to change the provider");
            return 64;
        }
        if let Some(requested) = options.model.as_deref()
            && requested != previous.model()
        {
            eprintln!(
                "error: --incremental cannot change the model from `{}` to `{requested}`",
                previous.model()
            );
            eprintln!("help: rebuild without --incremental to change the model");
            return 64;
        }
        if let Some(requested) = options.dimensions
            && requested != previous.dimensions()
        {
            eprintln!(
                "error: --incremental cannot change dimensions from {} to {requested}",
                previous.dimensions()
            );
            eprintln!("help: rebuild without --incremental to change dimensions");
            return 64;
        }
        let provider_name = previous.provider().to_string();
        let model = previous.model().to_string();
        let dimensions = previous.dimensions();
        let plan = match previous.plan_incremental(
            provider_name.clone(),
            model.clone(),
            dimensions,
            chunks,
        ) {
            Ok(plan) => plan,
            Err(error) => return report_error(&error),
        };
        let stats = plan.stats().clone();
        let (index, usage) = if stats.pending_chunks == 0 {
            match plan.finish(Vec::new()) {
                Ok(index) => (index, None),
                Err(error) => return report_error(&error),
            }
        } else {
            let pending = plan.pending_chunks_owned();
            let provider = match build_embedding_provider(
                &config,
                &provider_name,
                &model,
                dimensions,
            ) {
                Ok(provider) => provider,
                Err(error) => return report_error(&error),
            };
            let mut budget = Budget::default();
            let (vectors, usage) = match ai::embed_chunks(
                provider.as_ref(),
                &model,
                &pending,
                Some(dimensions),
                &mut budget,
            ) {
                Ok(result) => result,
                Err(error) => return report_error(&error),
            };
            match plan.finish(vectors) {
                Ok(index) => (index, Some(usage)),
                Err(error) => return report_error(&error),
            }
        };
        if let Err(error) = index.save(out) {
            return report_error(&error);
        }
        let bytes = std::fs::metadata(out).map(|metadata| metadata.len()).unwrap_or(0);
        let prompt_tokens = usage.as_ref().map_or(0, |usage| usage.prompt_tokens);
        let total_tokens = usage.as_ref().map_or(0, |usage| usage.total_tokens);
        if options.json {
            println!(
                "{}",
                ai::Json::object([
                    ("index", ai::Json::from(out.to_string_lossy().to_string())),
                    ("mode", ai::Json::from("semantic")),
                    ("incremental", ai::Json::from(true)),
                    ("provider", ai::Json::from(index.provider().to_string())),
                    ("model", ai::Json::from(index.model().to_string())),
                    (
                        "dimensions",
                        ai::Json::from(u64::try_from(index.dimensions()).unwrap_or(u64::MAX)),
                    ),
                    (
                        "documents",
                        ai::Json::from(u64::try_from(documents.len()).unwrap_or(u64::MAX)),
                    ),
                    (
                        "chunks",
                        ai::Json::from(u64::try_from(index.len()).unwrap_or(u64::MAX)),
                    ),
                    (
                        "vectors",
                        ai::Json::from(u64::try_from(index.len()).unwrap_or(u64::MAX)),
                    ),
                    (
                        "previous_chunks",
                        ai::Json::from(u64::try_from(stats.previous_chunks).unwrap_or(u64::MAX)),
                    ),
                    (
                        "reused_chunks",
                        ai::Json::from(u64::try_from(stats.reused_chunks).unwrap_or(u64::MAX)),
                    ),
                    (
                        "embedded_chunks",
                        ai::Json::from(u64::try_from(stats.pending_chunks).unwrap_or(u64::MAX)),
                    ),
                    (
                        "removed_chunks",
                        ai::Json::from(u64::try_from(stats.removed_chunks).unwrap_or(u64::MAX)),
                    ),
                    (
                        "removed_sources",
                        ai::Json::from(u64::try_from(stats.removed_sources).unwrap_or(u64::MAX)),
                    ),
                    (
                        "added_sources",
                        ai::Json::from(u64::try_from(stats.added_sources).unwrap_or(u64::MAX)),
                    ),
                    ("bytes", ai::Json::from(bytes)),
                    (
                        "usage",
                        ai::Json::object([
                            ("prompt_tokens", ai::Json::from(prompt_tokens)),
                            ("total_tokens", ai::Json::from(total_tokens)),
                        ]),
                    ),
                ])
            );
        } else {
            println!("index: {}", out.display());
            println!("mode: semantic (stored vectors enable semantic and hybrid search)");
            println!("update: incremental");
            println!("provider: {}", index.provider());
            println!("model: {}", index.model());
            println!("dimensions: {}", index.dimensions());
            println!("documents: {}", documents.len());
            println!("chunks: {}", index.len());
            println!("vectors: {}", index.len());
            println!(
                "reused: {} chunks; embedded: {} chunks; removed: {} chunks",
                stats.reused_chunks, stats.pending_chunks, stats.removed_chunks
            );
            println!(
                "sources: {} current, {} removed, {} added",
                stats.current_sources, stats.removed_sources, stats.added_sources
            );
            println!("bytes: {bytes}");
            println!("usage: prompt_tokens={prompt_tokens} total_tokens={total_tokens}");
            if provider_name == "hash" {
                println!(
                    "note: the hash provider is a deterministic lexical feature baseline, not a semantic model"
                );
            }
        }
        return 0;
    }

    if chunks.is_empty() {
        eprintln!("error: no indexable documents were found under `{}`", root.display());
        eprintln!("help: check --ext and the file contents, or use --mode lexical for an empty index");
        return 64;
    }
    let provider = match build_embedding_provider(&config, &provider_name, &model, hash_dimensions) {
        Ok(provider) => provider,
        Err(error) => return report_error(&error),
    };
    let mut budget = Budget::default();
    let (vectors, usage) = match ai::embed_chunks(
        provider.as_ref(),
        &model,
        &chunks,
        dimensions,
        &mut budget,
    ) {
        Ok(result) => result,
        Err(error) => return report_error(&error),
    };
    let actual_dimensions = vectors.first().map_or(hash_dimensions, ai::Vector::len);
    let index = match ai::RagIndex::new(
        provider.id(),
        &model,
        actual_dimensions,
        chunks,
        vectors,
    ) {
        Ok(index) => index,
        Err(error) => return report_error(&error),
    };
    if let Err(error) = index.save(out) {
        return report_error(&error);
    }
    let bytes = std::fs::metadata(out).map(|metadata| metadata.len()).unwrap_or(0);
    if options.json {
        println!(
            "{}",
            ai::Json::object([
                ("index", ai::Json::from(out.to_string_lossy().to_string())),
                ("mode", ai::Json::from("semantic")),
                ("incremental", ai::Json::from(false)),
                ("provider", ai::Json::from(index.provider().to_string())),
                ("model", ai::Json::from(index.model().to_string())),
                (
                    "dimensions",
                    ai::Json::from(u64::try_from(index.dimensions()).unwrap_or(u64::MAX)),
                ),
                (
                    "documents",
                    ai::Json::from(u64::try_from(documents.len()).unwrap_or(u64::MAX)),
                ),
                (
                    "chunks",
                    ai::Json::from(u64::try_from(index.len()).unwrap_or(u64::MAX)),
                ),
                (
                    "vectors",
                    ai::Json::from(u64::try_from(index.len()).unwrap_or(u64::MAX)),
                ),
                ("bytes", ai::Json::from(bytes)),
                (
                    "usage",
                    ai::Json::object([
                        (
                            "prompt_tokens",
                            ai::Json::from(usage.prompt_tokens),
                        ),
                        ("total_tokens", ai::Json::from(usage.total_tokens)),
                    ]),
                ),
            ])
        );
    } else {
        println!("index: {}", out.display());
        println!("mode: semantic (stored vectors enable semantic and hybrid search)");
        println!("provider: {}", index.provider());
        println!("model: {}", index.model());
        println!("dimensions: {}", index.dimensions());
        println!("documents: {}", documents.len());
        println!("chunks: {}", index.len());
        println!("vectors: {}", index.len());
        println!("bytes: {bytes}");
        println!(
            "usage: prompt_tokens={} total_tokens={}",
            usage.prompt_tokens, usage.total_tokens
        );
        if provider.id() == "hash" {
            println!(
                "note: the hash provider is a deterministic lexical feature baseline, not a semantic model"
            );
        }
    }
    0
}

fn rag_search(args: &[String]) -> i32 {
    let options = match parse_options(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("error: {message}");
            return 64;
        }
    };
    if options.help {
        print!("{AI_USAGE}");
        return 0;
    }
    if options.metrics.is_some() {
        eprintln!("error: --metrics is not supported by `vantyx ai search` yet");
        return 64;
    }
    let Some(index_path) = options.index.as_ref() else {
        eprintln!("error: `vantyx ai search` requires --index <file>");
        return 64;
    };
    let index = match ai::RagIndex::load(index_path) {
        Ok(index) => index,
        Err(error) => return report_error(&error),
    };
    let query = match options.query.clone() {
        Some(query) => query,
        None if !options.positional.is_empty() => options.positional.join(" "),
        None => {
            eprintln!("error: `vantyx ai search` requires --query <text>");
            return 64;
        }
    };
    if query.trim().is_empty() {
        eprintln!("error: search query must not be empty");
        return 64;
    }
    let mode = match search_mode(options.mode.as_deref(), &index) {
        Ok(mode) => mode,
        Err(message) => {
            eprintln!("error: {message}");
            return 64;
        }
    };
    let top_k = options.top_k.unwrap_or(5);
    let config = resolve_config(&options);
    let query_vector = if mode == ai::SearchMode::Lexical {
        None
    } else {
        let provider_name = options
            .provider
            .clone()
            .unwrap_or_else(|| index_provider_name(&index).to_string());
        let model = options
            .model
            .clone()
            .unwrap_or_else(|| index.model().to_string());
        let dimensions = options.dimensions.unwrap_or(index.dimensions());
        let provider = match build_embedding_provider(&config, &provider_name, &model, dimensions) {
            Ok(provider) => provider,
            Err(error) => return report_error(&error),
        };
        if options.provider.is_some() && provider.id() != index.provider() {
            eprintln!(
                "warning: query provider `{}` differs from index provider `{}`",
                provider.id(),
                index.provider()
            );
        }
        let mut budget = Budget::default();
        match ai::embed_query(provider.as_ref(), &model, &query, options.dimensions, &mut budget) {
            Ok(vector) => Some(vector),
            Err(error) => return report_error(&error),
        }
    };
    let hits = match if options.diverse {
        index.search_with_source_diverse(
            &query,
            query_vector.as_ref(),
            mode,
            top_k,
            options.source_prefix.as_deref(),
            ai::DEFAULT_MMR_LAMBDA,
        )
    } else {
        index.search_with_source(
            &query,
            query_vector.as_ref(),
            mode,
            top_k,
            options.source_prefix.as_deref(),
        )
    } {
        Ok(hits) => hits,
        Err(error) => return report_error(&error),
    };
    if options.json {
        let entries = hits
            .iter()
            .map(|hit| {
                ai::Json::object([
                    ("id", ai::Json::from(hit.id.clone())),
                    ("source", ai::Json::from(hit.source.clone())),
                    (
                        "start_line",
                        ai::Json::from(u64::try_from(hit.start_line).unwrap_or(u64::MAX)),
                    ),
                    (
                        "end_line",
                        ai::Json::from(u64::try_from(hit.end_line).unwrap_or(u64::MAX)),
                    ),
                    ("score", ai::Json::from(f64::from(hit.score))),
                    (
                        "lexical_score",
                        hit.lexical_score
                            .map_or(ai::Json::Null, |score| ai::Json::from(f64::from(score))),
                    ),
                    (
                        "semantic_score",
                        hit.semantic_score
                            .map_or(ai::Json::Null, |score| ai::Json::from(f64::from(score))),
                    ),
                    (
                        "lexical_rank",
                        hit.lexical_rank
                            .map_or(ai::Json::Null, |rank| ai::Json::from(rank as u64)),
                    ),
                    (
                        "semantic_rank",
                        hit.semantic_rank
                            .map_or(ai::Json::Null, |rank| ai::Json::from(rank as u64)),
                    ),
                    ("text", ai::Json::from(hit.text.clone())),
                ])
            })
            .collect();
        println!(
            "{}",
            ai::Json::object([
                (
                    "index",
                    ai::Json::from(index_path.to_string_lossy().to_string()),
                ),
                ("mode", ai::Json::from(mode.as_str())),
                (
                    "rerank",
                    ai::Json::from(if options.diverse { "mmr" } else { "none" }),
                ),
                ("query", ai::Json::from(query)),
                ("hits", ai::Json::Array(entries)),
            ])
        );
        return 0;
    }
    println!("index: {}", index_path.display());
    println!("mode: {}", mode.as_str());
    if options.diverse {
        println!("rerank: mmr");
    }
    println!("query: {query}");
    println!("hits: {}", hits.len());
    for (position, hit) in hits.iter().enumerate() {
        let excerpt = hit
            .text
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(160)
            .collect::<String>();
        println!(
            "[{}] {}:{}-{} score={:.6}",
            position + 1,
            hit.source,
            hit.start_line,
            hit.end_line,
            hit.score
        );
        println!("    {excerpt}");
    }
    0
}

fn rag_eval(args: &[String]) -> i32 {
    let options = match parse_options(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("error: {message}");
            return 64;
        }
    };
    if options.help {
        print!("{AI_USAGE}");
        return 0;
    }
    if options.metrics.is_some() {
        eprintln!("error: --metrics is not supported by `vantyx ai eval` yet");
        return 64;
    }
    if options.source_prefix.is_some() {
        eprintln!("error: --source-prefix is only supported by `vantyx ai search`");
        return 64;
    }
    if options.diverse {
        eprintln!("error: --diverse is only supported by `vantyx ai search`");
        return 64;
    }
    let Some(index_path) = options.index.as_ref() else {
        eprintln!("error: `vantyx ai eval` requires --index <file>");
        return 64;
    };
    let Some(fixture_path) = options.fixture.as_ref() else {
        eprintln!("error: `vantyx ai eval` requires --fixture <file>");
        return 64;
    };
    let index = match ai::RagIndex::load(index_path) {
        Ok(index) => index,
        Err(error) => return report_error(&error),
    };
    let metadata = match std::fs::metadata(fixture_path) {
        Ok(metadata) => metadata,
        Err(error) => {
            eprintln!(
                "error: cannot read fixture `{}`: {error}",
                fixture_path.display()
            );
            return 1;
        }
    };
    if metadata.len() > ai::rag::MAX_INDEX_BYTES as u64 {
        eprintln!(
            "error: fixture `{}` is {} bytes; the limit is {}",
            fixture_path.display(),
            metadata.len(),
            ai::rag::MAX_INDEX_BYTES
        );
        return 64;
    }
    let text = match std::fs::read_to_string(fixture_path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "error: cannot read fixture `{}`: {error}",
                fixture_path.display()
            );
            return 1;
        }
    };
    let value = match ai::parse(&text) {
        Ok(value) => value,
        Err(error) => {
            eprintln!(
                "error: fixture `{}` is not valid JSON: {error}",
                fixture_path.display()
            );
            return 64;
        }
    };
    let queries = match ai::parse_eval_queries(&value) {
        Ok(queries) => queries,
        Err(error) => return report_error(&error),
    };
    let mode = match search_mode(options.mode.as_deref(), &index) {
        Ok(mode) => mode,
        Err(message) => {
            eprintln!("error: {message}");
            return 64;
        }
    };
    let k = options.top_k.unwrap_or(5);
    let config = resolve_config(&options);
    let mut budget = Budget::default();
    let provider = if mode == ai::SearchMode::Lexical {
        None
    } else {
        let provider_name = options
            .provider
            .clone()
            .unwrap_or_else(|| index_provider_name(&index).to_string());
        let model = options
            .model
            .clone()
            .unwrap_or_else(|| index.model().to_string());
        let dimensions = options.dimensions.unwrap_or(index.dimensions());
        match build_embedding_provider(&config, &provider_name, &model, dimensions) {
            Ok(provider) => Some(provider),
            Err(error) => return report_error(&error),
        }
    };
    let model = options
        .model
        .clone()
        .unwrap_or_else(|| index.model().to_string());
    let mut encoder = |query: &str| -> Result<ai::Vector, ai::AiError> {
        let Some(provider) = provider.as_ref() else {
            return Err(ai::AiError::new(
                ai::provider::AI_E_INVALID_REQUEST,
                "lexical evaluation must not embed queries",
            ));
        };
        ai::embed_query(provider.as_ref(), &model, query, options.dimensions, &mut budget)
    };
    let report = match ai::evaluate(&index, &queries, mode, k, &mut encoder) {
        Ok(report) => report,
        Err(error) => return report_error(&error),
    };
    if options.json {
        println!("{}", report.to_json());
    } else {
        println!("mode: {}", report.mode.as_str());
        println!("k: {}", report.k);
        println!("queries: {}", report.queries);
        println!("recall_at_k: {:.4}", report.recall_at_k);
        println!("precision_at_k: {:.4}", report.precision_at_k);
        println!("mrr: {:.4}", report.mrr);
        for result in &report.per_query {
            println!(
                "- {} | recall={:.4} precision={:.4} rr={:.4}",
                result.query, result.recall_at_k, result.precision_at_k, result.reciprocal_rank
            );
        }
        println!(
            "note: retrieval metrics only; they do not prove that a generated answer is grounded"
        );
    }
    0
}

fn search_mode(
    requested: Option<&str>,
    index: &ai::RagIndex,
) -> Result<ai::SearchMode, String> {
    let mode = match requested {
        Some(name) => ai::SearchMode::parse(name)
            .map_err(|error| format!("unknown search mode: {error}"))?,
        None if index.has_vectors() => ai::SearchMode::Hybrid,
        None => ai::SearchMode::Lexical,
    };
    if mode != ai::SearchMode::Lexical && !index.has_vectors() {
        return Err(
            "this index has no vectors; rebuild it without --mode lexical or use --mode lexical"
                .to_string(),
        );
    }
    Ok(mode)
}

fn index_provider_name(index: &ai::RagIndex) -> &str {
    match index.provider() {
        "lexical" | "none" => "hash",
        name => name,
    }
}

fn build_embedding_provider(
    config: &AiConfig,
    provider: &str,
    model: &str,
    dimensions: usize,
) -> Result<Box<dyn Provider>, ai::AiError> {
    match provider {
        "mock" | "hash" => HashEmbeddingProvider::new(dimensions)
            .map(|provider| Box::new(provider) as Box<dyn Provider>),
        "openai" => Ok(Box::new(build_openai(config, model)?)),
        other => Err(ai::AiError::new(
            ai::provider::AI_E_CONFIG,
            format!(
                "unknown embedding provider `{other}`; expected mock, hash, or openai"
            ),
        )),
    }
}

fn embedding_inputs(options: &Options) -> Result<Vec<String>, i32> {
    let inputs = if options.prompt_stdin {
        read_stdin_lines()?
    } else if !options.positional.is_empty() {
        options.positional.clone()
    } else if !io::stdin().is_terminal() {
        read_stdin_lines()?
    } else {
        eprintln!("error: provide text, multiple texts, or `-` to read one input per line");
        return Err(64);
    };
    if inputs.is_empty() {
        eprintln!("error: no non-empty embedding inputs were provided");
        return Err(64);
    }
    Ok(inputs)
}

fn read_stdin_lines() -> Result<Vec<String>, i32> {
    let text = read_stdin()?;
    let inputs: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect();
    if inputs.is_empty() {
        eprintln!("error: standard input contained no non-empty lines");
        return Err(64);
    }
    Ok(inputs)
}

fn build_messages(options: &Options) -> Result<Vec<Message>, i32> {
    let mut messages = Vec::new();
    if let Some(system) = &options.system {
        messages.push(Message::system(system.clone()));
    }
    for message in &options.messages {
        messages.push(Message::user(message.clone()));
    }
    if !options.positional.is_empty() {
        messages.push(Message::user(options.positional.join(" ")));
    }
    if options.prompt_stdin || messages.is_empty() {
        if !options.prompt_stdin && io::stdin().is_terminal() {
            eprintln!("error: provide a prompt, `--message`, or `-` to read standard input");
            return Err(64);
        }
        match read_stdin() {
            Ok(text) if !text.trim().is_empty() => messages.push(Message::user(text)),
            Ok(_) => {
                eprintln!("error: standard input was empty");
                return Err(64);
            }
            Err(code) => return Err(code),
        }
    }
    Ok(messages)
}

fn load_schema(argument: &str) -> Result<ai::Json, String> {
    const MAX_SCHEMA_FILE: u64 = 1024 * 1024;
    let text = if argument.trim_start().starts_with('{') {
        argument.to_string()
    } else {
        let path = PathBuf::from(argument);
        let metadata = std::fs::metadata(&path)
            .map_err(|error| format!("cannot read schema `{}`: {error}", path.display()))?;
        if metadata.len() > MAX_SCHEMA_FILE {
            return Err(format!(
                "schema `{}` is {} bytes; the limit is {MAX_SCHEMA_FILE}",
                path.display(),
                metadata.len()
            ));
        }
        std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read schema `{}`: {error}", path.display()))?
    };
    if text.len() > MAX_SCHEMA_FILE as usize {
        return Err(format!(
            "schema is {} bytes; the limit is {MAX_SCHEMA_FILE}",
            text.len()
        ));
    }
    let value =
        ai::json::parse(&text).map_err(|error| format!("schema is not valid JSON: {error}"))?;
    if !matches!(value, ai::Json::Object(_)) {
        return Err("schema must be a JSON object".to_string());
    }
    Ok(value)
}

fn tokens(args: &[String]) -> i32 {
    let mut text = args.join(" ");
    if text.is_empty() || text == "-" {
        if io::stdin().is_terminal() {
            eprintln!("error: provide text or pipe it on standard input");
            return 64;
        }
        match read_stdin() {
            Ok(input) => text = input,
            Err(code) => return code,
        }
    }
    if text.trim().is_empty() {
        eprintln!("error: no text to estimate");
        return 64;
    }
    println!(
        "estimate: {} tokens (labelled estimate, not a model tokenizer)",
        ai::estimate_tokens(&text)
    );
    0
}

fn resolve_config(options: &Options) -> AiConfig {
    let mut config = AiConfig::from_env();
    if let Some(provider) = &options.provider {
        config.provider = provider.clone();
    }
    if let Some(model) = &options.model {
        config.model = model.clone();
    }
    if let Some(endpoint) = &options.endpoint {
        config.endpoint = endpoint.clone();
    }
    if let Some(api_key_env) = &options.api_key_env {
        config.api_key_env = api_key_env.clone();
    }
    if options.allow_network {
        config.allow_network = true;
    }
    if let Some(timeout_ms) = options.timeout_ms {
        config.timeout = Duration::from_millis(timeout_ms);
    }
    config
}

fn build_provider(
    config: &AiConfig,
    provider: &str,
    model: &str,
) -> Result<Box<dyn Provider>, ai::AiError> {
    match provider {
        "mock" => Ok(Box::new(MockProvider::new(
            "mock",
            vec![Ok(ChatResponse::text(
                model,
                "[mock] offline response; no network call or model was used",
            ))],
        ))),
        "echo" => Ok(Box::new(EchoProvider::new())),
        "openai" => Ok(Box::new(build_openai(config, model)?)),
        other => Err(ai::AiError::new(
            ai::provider::AI_E_CONFIG,
            format!(
                "unknown provider `{other}`; expected mock, echo, or openai (`hash` is embed-only)"
            ),
        )),
    }
}

fn build_openai(config: &AiConfig, _model: &str) -> Result<OpenAiProvider, ai::AiError> {
    let api_key = config.api_key()?;
    let transport = HttpTransport::new(config.network_policy())
        .with_timeout(config.timeout)
        .with_max_response_bytes(config.max_response_bytes);
    Ok(OpenAiProvider::new(
        config.endpoint.clone(),
        api_key,
        Box::new(transport),
    ))
}

fn capabilities(capabilities: &ProviderCapabilities) -> String {
    let mut names = Vec::new();
    if capabilities.chat {
        names.push("chat".to_string());
    }
    if capabilities.streaming {
        names.push("streaming".to_string());
    }
    if capabilities.tools {
        names.push("tools".to_string());
    }
    if capabilities.json_object {
        names.push("json_object".to_string());
    }
    if capabilities.json_schema {
        names.push("json_schema".to_string());
    }
    if capabilities.embeddings {
        names.push("embeddings".to_string());
    }
    if capabilities.vision {
        names.push("vision".to_string());
    }
    if capabilities.audio_input {
        names.push("audio_in".to_string());
    }
    if capabilities.audio_output {
        names.push("audio_out".to_string());
    }
    if capabilities.seed {
        names.push("seed".to_string());
    }
    if let Some(tokens) = capabilities.max_context_tokens {
        names.push(format!("context={tokens}"));
    }
    if names.is_empty() {
        "none".to_string()
    } else {
        names.join(", ")
    }
}

fn report_error(error: &ai::AiError) -> i32 {
    eprintln!("error: {error}");
    if error.retryable {
        eprintln!("note: this failure is retryable");
    }
    if error.code == ai::provider::AI_E_NETWORK_DISABLED {
        eprintln!("help: pass --allow-network for remote HTTPS, or use a loopback endpoint");
    }
    if error.code == ai::provider::AI_E_CONNECT {
        eprintln!(
            "help: check that the local server is running, or set --endpoint to a reachable URL"
        );
    }
    if error.code == ai::provider::AI_E_SCHEMA {
        eprintln!("help: tighten the schema or prompt, or raise --repair-attempts (max 3)");
    }
    if error.code == ai::provider::AI_E_JSON {
        eprintln!("help: use a model with JSON mode, or raise --repair-attempts (max 3)");
    }
    1
}

fn read_stdin() -> Result<String, i32> {
    let mut text = String::new();
    io::stdin().read_to_string(&mut text).map_err(|error| {
        eprintln!("error: cannot read standard input: {error}");
        1
    })?;
    if text.len() > 1024 * 1024 {
        eprintln!("error: standard input exceeds the 1 MiB prompt limit");
        return Err(64);
    }
    Ok(text)
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag, Some(value)),
            _ => (arg.as_str(), None),
        };
        match flag {
            "-h" | "--help" => options.help = true,
            "--allow-network" => {
                if inline.is_some() {
                    return Err("--allow-network does not take a value".into());
                }
                options.allow_network = true;
            }
            "--json" => {
                if inline.is_some() {
                    return Err("--json does not take a value".into());
                }
                options.json = true;
            }
            "--incremental" => {
                if inline.is_some() {
                    return Err("--incremental does not take a value".into());
                }
                options.incremental = true;
            }
            "--diverse" => {
                if inline.is_some() {
                    return Err("--diverse does not take a value".into());
                }
                options.diverse = true;
            }
            "--stream" => {
                if inline.is_some() {
                    return Err("--stream does not take a value".into());
                }
                options.stream = true;
            }
            "--provider" => options.provider = Some(take_value(flag, inline, args, &mut index)?),
            "--model" => options.model = Some(take_value(flag, inline, args, &mut index)?),
            "--endpoint" => options.endpoint = Some(take_value(flag, inline, args, &mut index)?),
            "--api-key-env" => {
                options.api_key_env = Some(take_value(flag, inline, args, &mut index)?);
            }
            "--system" => options.system = Some(take_value(flag, inline, args, &mut index)?),
            "--message" => options
                .messages
                .push(take_value(flag, inline, args, &mut index)?),
            "--metrics" => {
                options.metrics = Some(PathBuf::from(take_value(flag, inline, args, &mut index)?));
            }
            "--schema" => options.schema = Some(take_value(flag, inline, args, &mut index)?),
            "--repair-attempts" => {
                let value = take_value(flag, inline, args, &mut index)?;
                options.repair_attempts = Some(
                    value
                        .parse::<usize>()
                        .ok()
                        .filter(|value| *value <= ai::MAX_REPAIR_ATTEMPTS)
                        .ok_or_else(|| {
                            format!(
                                "--repair-attempts requires an integer from 0 to {}",
                                ai::MAX_REPAIR_ATTEMPTS
                            )
                        })?,
                );
            }
            "--response" => options
                .responses
                .push(take_value(flag, inline, args, &mut index)?),
            "--root" => options.root = Some(PathBuf::from(take_value(flag, inline, args, &mut index)?)),
            "--out" => options.out = Some(PathBuf::from(take_value(flag, inline, args, &mut index)?)),
            "--index" => {
                options.index = Some(PathBuf::from(take_value(flag, inline, args, &mut index)?));
            }
            "--query" => options.query = Some(take_value(flag, inline, args, &mut index)?),
            "--source-prefix" => {
                let value = take_value(flag, inline, args, &mut index)?;
                if value.is_empty() {
                    return Err("--source-prefix must not be empty".to_string());
                }
                options.source_prefix = Some(value);
            }
            "--fixture" => {
                options.fixture = Some(PathBuf::from(take_value(flag, inline, args, &mut index)?));
            }
            "--mode" => options.mode = Some(take_value(flag, inline, args, &mut index)?),
            "--top-k" => {
                let value = take_value(flag, inline, args, &mut index)?;
                options.top_k = Some(
                    value
                        .parse::<usize>()
                        .ok()
                        .filter(|value| (1..=ai::rag::MAX_TOP_K).contains(value))
                        .ok_or_else(|| {
                            format!(
                                "--top-k requires an integer from 1 to {}",
                                ai::rag::MAX_TOP_K
                            )
                        })?,
                );
            }
            "--chunk-chars" => {
                let value = take_value(flag, inline, args, &mut index)?;
                options.chunk_chars = Some(
                    value
                        .parse::<usize>()
                        .ok()
                        .filter(|value| {
                            (ai::rag::MIN_CHUNK_CHARS..=ai::rag::MAX_CHUNK_CHARS)
                                .contains(value)
                        })
                        .ok_or_else(|| {
                            format!(
                                "--chunk-chars requires an integer from {} to {}",
                                ai::rag::MIN_CHUNK_CHARS, ai::rag::MAX_CHUNK_CHARS
                            )
                        })?,
                );
            }
            "--overlap-chars" => {
                let value = take_value(flag, inline, args, &mut index)?;
                options.overlap_chars = Some(
                    value
                        .parse::<usize>()
                        .ok()
                        .filter(|value| *value <= ai::rag::MAX_OVERLAP_CHARS)
                        .ok_or_else(|| {
                            format!(
                                "--overlap-chars requires an integer from 0 to {}",
                                ai::rag::MAX_OVERLAP_CHARS
                            )
                        })?,
                );
            }
            "--ext" => {
                for extension in take_value(flag, inline, args, &mut index)?.split(',') {
                    let extension = extension.trim().trim_start_matches('.').to_ascii_lowercase();
                    if extension.is_empty() || !extension.chars().all(|c| c.is_ascii_alphanumeric()) {
                        return Err(format!(
                            "--ext requires an alphanumeric extension, got `{extension}`"
                        ));
                    }
                    options.extensions.push(extension);
                }
            }
            "--timeout-ms" => {
                let value = take_value(flag, inline, args, &mut index)?;
                options.timeout_ms = Some(
                    value
                        .parse::<u64>()
                        .ok()
                        .filter(|value| *value > 0)
                        .ok_or_else(|| "--timeout-ms requires a positive integer".to_string())?,
                );
            }
            "--max-tokens" => {
                let value = take_value(flag, inline, args, &mut index)?;
                options.max_tokens = Some(
                    value
                        .parse::<u32>()
                        .ok()
                        .filter(|value| *value > 0)
                        .ok_or_else(|| "--max-tokens requires a positive integer".to_string())?,
                );
            }
            "--temperature" => {
                let value = take_value(flag, inline, args, &mut index)?;
                options.temperature = Some(
                    value
                        .parse::<f64>()
                        .map_err(|_| "--temperature requires a number".to_string())?,
                );
            }
            "--dimensions" => {
                let value = take_value(flag, inline, args, &mut index)?;
                options.dimensions = Some(
                    value
                        .parse::<usize>()
                        .ok()
                        .filter(|value| (1..=ai::MAX_EMBEDDING_DIMENSIONS).contains(value))
                        .ok_or_else(|| {
                            format!(
                                "--dimensions requires an integer from 1 to {}",
                                ai::MAX_EMBEDDING_DIMENSIONS
                            )
                        })?,
                );
            }
            "-" => options.prompt_stdin = true,
            other if other.starts_with('-') => {
                return Err(format!("unknown option `{other}`; try `vantyx ai --help`"));
            }
            other => options.positional.push(other.to_string()),
        }
        index += 1;
    }
    if options.prompt_stdin && !options.positional.is_empty() {
        return Err("use either `-` or a positional prompt, not both".into());
    }
    Ok(options)
}

fn take_value(
    flag: &str,
    inline: Option<&str>,
    args: &[String],
    index: &mut usize,
) -> Result<String, String> {
    if let Some(value) = inline {
        if value.is_empty() {
            return Err(format!("{flag} requires a value"));
        }
        return Ok(value.to_string());
    }
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn parses_chat_options() {
        let options = parse_options(&args(&[
            "--provider",
            "openai",
            "--model=qwen",
            "--message",
            "one",
            "--message=two",
            "--temperature",
            "0.2",
            "--allow-network",
            "--json",
        ]))
        .unwrap();
        assert_eq!(options.provider.as_deref(), Some("openai"));
        assert_eq!(options.model.as_deref(), Some("qwen"));
        assert_eq!(options.messages, vec!["one", "two"]);
        assert_eq!(options.temperature, Some(0.2));
        assert!(options.allow_network);
        assert!(options.json);
    }

    #[test]
    fn parses_the_stream_flag() {
        let options = parse_options(&args(&["--stream", "hello"])).unwrap();
        assert!(options.stream);
        assert_eq!(options.positional, vec!["hello"]);
        assert!(parse_options(&args(&["--stream=true"])).is_err());
    }

    #[test]
    fn parses_the_incremental_index_flag() {
        let options = parse_options(&args(&[
            "index",
            "--root",
            "docs",
            "--out",
            "index.json",
            "--incremental",
        ]))
        .unwrap();
        assert!(options.incremental);
        assert!(!options.json);
        assert!(parse_options(&args(&["--incremental=true"])).is_err());
    }

    #[test]
    fn parses_the_diverse_search_flag() {
        let options = parse_options(&args(&["--diverse", "--query", "memory"])).unwrap();
        assert!(options.diverse);
        assert_eq!(options.query.as_deref(), Some("memory"));
        assert!(parse_options(&args(&["--diverse=true"])).is_err());
    }

    #[test]
    fn parses_embedding_dimensions() {
        let options = parse_options(&args(&[
            "--provider",
            "hash",
            "--dimensions",
            "16",
            "--json",
            "one",
            "two",
        ]))
        .unwrap();
        assert_eq!(options.provider.as_deref(), Some("hash"));
        assert_eq!(options.dimensions, Some(16));
        assert_eq!(options.positional, vec!["one", "two"]);
        assert!(parse_options(&args(&["--dimensions", "0"])).is_err());
        assert!(parse_options(&args(&["--dimensions", "4097"])).is_err());
    }

    #[test]
    fn parses_extract_options() {
        let options = parse_options(&args(&[
            "--provider",
            "mock",
            "--schema",
            "schema.json",
            "--repair-attempts",
            "2",
            "--response",
            "{}",
            "--response",
            "{\"a\":1}",
        ]))
        .unwrap();
        assert_eq!(options.schema.as_deref(), Some("schema.json"));
        assert_eq!(options.repair_attempts, Some(2));
        assert_eq!(options.responses.len(), 2);
        assert!(parse_options(&args(&["--repair-attempts", "4"])).is_err());
    }

    #[test]
    fn rejects_unknown_options_and_bad_numbers() {
        assert!(parse_options(&args(&["--wat"])).is_err());
        assert!(parse_options(&args(&["--max-tokens", "0"])).is_err());
        assert!(parse_options(&args(&["--timeout-ms", "nope"])).is_err());
        assert!(parse_options(&args(&["--provider"])).is_err());
    }

    #[test]
    fn capabilities_are_reported_conservatively() {
        let text = capabilities(&ProviderCapabilities::openai_compatible());
        assert!(text.contains("chat"));
        assert!(text.contains("streaming"));
        assert!(text.contains("embeddings"));
        let mock = capabilities(&ProviderCapabilities::mock());
        assert!(mock.contains("chat"));
        assert!(!mock.contains("streaming"));
        let hash = capabilities(&ProviderCapabilities::hash_embedding());
        assert!(hash.contains("embeddings"));
        assert!(!hash.contains("chat"));
    }
}
