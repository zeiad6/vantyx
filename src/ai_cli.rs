//! `vantyx ai` command implementation.
//!
//! The CLI is the supported AI surface until modules, async, and a standard
//! library exist. It never downloads models or reads secrets from project
//! files.

use std::io::{self, IsTerminal, Read};
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
    match provider.chat(&request, &mut budget) {
        Ok(response) => {
            if options.json {
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
        assert!(!text.contains("streaming"));
        assert!(text.contains("embeddings"));
        let hash = capabilities(&ProviderCapabilities::hash_embedding());
        assert!(hash.contains("embeddings"));
        assert!(!hash.contains("chat"));
    }
}
