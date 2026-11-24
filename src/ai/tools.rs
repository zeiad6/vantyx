//! Capability-gated tools with schema validation and deterministic limits.
//!
//! A model never inherits the host's permissions. A tool runs only when its
//! required capabilities are present in the caller's set, its arguments match
//! the declared schema, and its policy allows the call.

use super::json::Json;
use super::provider::{AI_E_CAPABILITY, AI_E_SCHEMA, AiError, ToolSpec};
use super::schema;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Capability {
    NetworkConnect(String),
    FsRead(PathBuf),
    FsWrite(PathBuf),
    EnvRead(String),
    SecretUse(String),
    ProcessSpawn,
    ShellExec,
    ToolCall(String),
}

#[derive(Clone, Debug, Default)]
pub struct CapabilitySet {
    entries: Vec<Capability>,
}

impl CapabilitySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allow(&mut self, capability: Capability) -> &mut Self {
        if !self.entries.contains(&capability) {
            self.entries.push(capability);
        }
        self
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[Capability] {
        &self.entries
    }

    pub fn covers(&self, required: &Capability) -> bool {
        self.entries.iter().any(|granted| covers(granted, required))
    }

    pub fn allows_tool(&self, name: &str) -> bool {
        self.covers(&Capability::ToolCall(name.to_string()))
    }

    pub fn allows_network(&self, endpoint: &str) -> bool {
        self.covers(&Capability::NetworkConnect(endpoint.to_string()))
    }

    pub fn allows_env(&self, name: &str) -> bool {
        self.covers(&Capability::EnvRead(name.to_string()))
    }

    pub fn allows_secret(&self, name: &str) -> bool {
        self.covers(&Capability::SecretUse(name.to_string()))
    }

    pub fn allows_read(&self, path: &Path) -> bool {
        self.entries.iter().any(|entry| match entry {
            Capability::FsRead(root) => path_within(root, path, true),
            _ => false,
        })
    }

    pub fn allows_write(&self, path: &Path) -> bool {
        self.entries.iter().any(|entry| match entry {
            Capability::FsWrite(root) => path_within(root, path, false),
            _ => false,
        })
    }
}

fn covers(granted: &Capability, required: &Capability) -> bool {
    match (granted, required) {
        (Capability::FsRead(granted), Capability::FsRead(required)) => {
            canonical_prefix(granted, required)
        }
        (Capability::FsWrite(granted), Capability::FsWrite(required)) => {
            canonical_prefix(granted, required)
        }
        (Capability::NetworkConnect(granted), Capability::NetworkConnect(required)) => {
            granted == required
        }
        (Capability::EnvRead(granted), Capability::EnvRead(required)) => granted == required,
        (Capability::SecretUse(granted), Capability::SecretUse(required)) => granted == required,
        (Capability::ToolCall(granted), Capability::ToolCall(required)) => granted == required,
        (Capability::ProcessSpawn, Capability::ProcessSpawn) => true,
        (Capability::ShellExec, Capability::ShellExec) => true,
        _ => false,
    }
}

fn canonical_prefix(granted: &Path, required: &Path) -> bool {
    let Ok(granted) = std::fs::canonicalize(granted) else {
        return false;
    };
    let required = if required.exists() {
        std::fs::canonicalize(required).map_err(|_| ())
    } else {
        required
            .parent()
            .and_then(|parent| std::fs::canonicalize(parent).ok())
            .map(|parent| parent.join(required.file_name().unwrap_or_default()))
            .ok_or(())
    };
    match required {
        Ok(required) => required.starts_with(granted),
        Err(()) => false,
    }
}

fn path_within(root: &Path, path: &Path, must_exist: bool) -> bool {
    let Ok(root) = std::fs::canonicalize(root) else {
        return false;
    };
    let candidate = if must_exist || path.exists() {
        std::fs::canonicalize(path).map_err(|_| ())
    } else {
        path.parent()
            .and_then(|parent| std::fs::canonicalize(parent).ok())
            .map(|parent| parent.join(path.file_name().unwrap_or_default()))
            .ok_or(())
    };
    match candidate {
        Ok(candidate) => candidate.starts_with(root),
        Err(()) => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Approval {
    Auto,
    Ask,
    Deny,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolPolicy {
    pub approval: Approval,
    pub max_calls: u32,
}

impl Default for ToolPolicy {
    fn default() -> Self {
        Self {
            approval: Approval::Auto,
            max_calls: 4,
        }
    }
}

impl ToolPolicy {
    pub fn auto(max_calls: u32) -> Self {
        Self {
            approval: Approval::Auto,
            max_calls,
        }
    }

    pub fn ask(max_calls: u32) -> Self {
        Self {
            approval: Approval::Ask,
            max_calls,
        }
    }

    pub fn deny() -> Self {
        Self {
            approval: Approval::Deny,
            max_calls: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolError {
    pub code: String,
    pub message: String,
}

impl ToolError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn capability(message: impl Into<String>) -> Self {
        Self::new(AI_E_CAPABILITY, message)
    }
}

impl fmt::Display for ToolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ToolError {}

impl From<ToolError> for AiError {
    fn from(error: ToolError) -> Self {
        AiError::new(error.code, error.message)
    }
}

pub struct ToolContext<'a> {
    pub capabilities: &'a CapabilitySet,
    pub approved: &'a [String],
    pub deadline: Option<Instant>,
}

impl ToolContext<'_> {
    pub fn check_deadline(&self) -> Result<(), ToolError> {
        if let Some(deadline) = self.deadline
            && Instant::now() >= deadline
        {
            return Err(ToolError::new("AI_E_TIMEOUT", "tool deadline exceeded"));
        }
        Ok(())
    }
}

pub trait Tool {
    fn spec(&self) -> ToolSpec;
    fn required_capabilities(&self) -> Vec<Capability>;
    fn call(&self, arguments: &Json, context: &ToolContext<'_>) -> Result<Json, ToolError>;
}

struct RegisteredTool {
    tool: Box<dyn Tool>,
    policy: ToolPolicy,
    calls: u32,
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: Vec<RegisteredTool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Box<dyn Tool>, policy: ToolPolicy) -> Option<()> {
        let spec = tool.spec();
        if self
            .tools
            .iter()
            .any(|registered| registered.tool.spec().name == spec.name)
        {
            return None;
        }
        self.tools.push(RegisteredTool {
            tool,
            policy,
            calls: 0,
        });
        Some(())
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Returns specs whose required capabilities are all granted.
    pub fn specs_for(&self, capabilities: &CapabilitySet) -> Vec<ToolSpec> {
        self.tools
            .iter()
            .filter(|registered| {
                registered
                    .tool
                    .required_capabilities()
                    .iter()
                    .all(|required| capabilities.covers(required))
            })
            .map(|registered| registered.tool.spec())
            .collect()
    }

    pub fn call(
        &mut self,
        name: &str,
        arguments: &Json,
        context: &ToolContext<'_>,
    ) -> Result<Json, ToolError> {
        context.check_deadline()?;
        let index = self
            .tools
            .iter()
            .position(|registered| registered.tool.spec().name == name)
            .ok_or_else(|| ToolError::capability(format!("unknown tool `{name}`")))?;

        let registered = &self.tools[index];
        match registered.policy.approval {
            Approval::Deny => {
                return Err(ToolError::capability(format!(
                    "tool `{name}` is denied by policy"
                )));
            }
            Approval::Ask if !context.approved.iter().any(|approved| approved == name) => {
                return Err(ToolError::capability(format!(
                    "tool `{name}` requires explicit approval"
                )));
            }
            Approval::Auto | Approval::Ask => {}
        }
        if registered.calls >= registered.policy.max_calls {
            return Err(ToolError::capability(format!(
                "tool `{name}` reached its call limit of {}",
                registered.policy.max_calls
            )));
        }
        for required in registered.tool.required_capabilities() {
            if !context.capabilities.covers(&required) {
                return Err(ToolError::capability(format!(
                    "tool `{name}` requires capability {required:?}"
                )));
            }
        }
        let spec = registered.tool.spec();
        schema::validate(&spec.parameters, arguments)
            .map_err(|error| ToolError::new(AI_E_SCHEMA, error.to_string()))?;
        let result = registered.tool.call(arguments, context)?;
        self.tools[index].calls += 1;
        Ok(result)
    }

    pub fn calls(&self, name: &str) -> u32 {
        self.tools
            .iter()
            .find(|registered| registered.tool.spec().name == name)
            .map_or(0, |registered| registered.calls)
    }
}

fn text_argument(arguments: &Json, name: &str) -> Result<String, ToolError> {
    arguments
        .get(name)
        .and_then(Json::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| ToolError::new(AI_E_SCHEMA, format!("missing string argument `{name}`")))
}

fn text_schema() -> Json {
    Json::object([
        ("type", Json::from("object")),
        (
            "properties",
            Json::object([("text", Json::object([("type", Json::from("string"))]))]),
        ),
        ("required", Json::Array(vec![Json::from("text")])),
        ("additionalProperties", Json::from(false)),
    ])
}

/// Returns its `text` argument unchanged. It requires no host capability.
pub struct EchoTool;

impl Tool for EchoTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "echo".into(),
            description: "Return the supplied text unchanged.".into(),
            parameters: text_schema(),
        }
    }

    fn required_capabilities(&self) -> Vec<Capability> {
        Vec::new()
    }

    fn call(&self, arguments: &Json, _context: &ToolContext<'_>) -> Result<Json, ToolError> {
        Ok(Json::object([(
            "text",
            Json::from(text_argument(arguments, "text")?),
        )]))
    }
}

/// Counts words and characters. It requires no host capability.
pub struct WordCountTool;

impl Tool for WordCountTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "word_count".into(),
            description: "Count words and characters in the supplied text.".into(),
            parameters: text_schema(),
        }
    }

    fn required_capabilities(&self) -> Vec<Capability> {
        Vec::new()
    }

    fn call(&self, arguments: &Json, _context: &ToolContext<'_>) -> Result<Json, ToolError> {
        let text = text_argument(arguments, "text")?;
        let words = text.split_whitespace().count();
        let characters = text.chars().count();
        Ok(Json::object([
            ("words", Json::from(words as u64)),
            ("characters", Json::from(characters as u64)),
        ]))
    }
}

/// Reads a UTF-8 text file under an explicit root. The capability check is
/// enforced both by the registry and by this tool.
pub struct ReadTextFileTool {
    root: PathBuf,
    max_bytes: usize,
}

impl ReadTextFileTool {
    pub fn new(root: impl Into<PathBuf>, max_bytes: usize) -> Self {
        Self {
            root: root.into(),
            max_bytes,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl Tool for ReadTextFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read_text_file".into(),
            description: "Read a UTF-8 text file under the configured root.".into(),
            parameters: Json::object([
                ("type", Json::from("object")),
                (
                    "properties",
                    Json::object([("path", Json::object([("type", Json::from("string"))]))]),
                ),
                ("required", Json::Array(vec![Json::from("path")])),
                ("additionalProperties", Json::from(false)),
            ]),
        }
    }

    fn required_capabilities(&self) -> Vec<Capability> {
        vec![Capability::FsRead(self.root.clone())]
    }

    fn call(&self, arguments: &Json, context: &ToolContext<'_>) -> Result<Json, ToolError> {
        context.check_deadline()?;
        let path = text_argument(arguments, "path")?;
        let candidate = PathBuf::from(&path);
        if !path_within(&self.root, &candidate, true) {
            return Err(ToolError::capability(format!(
                "path `{path}` is outside the allowed root"
            )));
        }
        let metadata = std::fs::metadata(&candidate)
            .map_err(|error| ToolError::new("AI_E_IO", format!("cannot stat `{path}`: {error}")))?;
        if metadata.len() > self.max_bytes as u64 {
            return Err(ToolError::new(
                "AI_E_RESPONSE_TOO_LARGE",
                format!("file exceeds the {} byte limit", self.max_bytes),
            ));
        }
        let text = std::fs::read_to_string(&candidate)
            .map_err(|error| ToolError::new("AI_E_IO", format!("cannot read `{path}`: {error}")))?;
        Ok(Json::object([
            ("path", Json::from(path)),
            ("text", Json::from(text)),
        ]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::json::parse;

    fn context<'a>(capabilities: &'a CapabilitySet, approved: &'a [String]) -> ToolContext<'a> {
        ToolContext {
            capabilities,
            approved,
            deadline: None,
        }
    }

    #[test]
    fn echo_tool_validates_and_returns_text() {
        let mut registry = ToolRegistry::new();
        assert!(
            registry
                .register(Box::new(EchoTool), ToolPolicy::auto(2))
                .is_some()
        );
        let capabilities = CapabilitySet::new();
        let approved = [];
        let ctx = context(&capabilities, &approved);
        let result = registry
            .call("echo", &parse(r#"{"text":"hi"}"#).unwrap(), &ctx)
            .unwrap();
        assert_eq!(result.get("text").unwrap().as_str(), Some("hi"));
        assert!(
            registry
                .call("echo", &parse(r#"{"text":1}"#).unwrap(), &ctx)
                .is_err()
        );
    }

    #[test]
    fn registry_filters_tools_by_capability() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Box::new(EchoTool), ToolPolicy::auto(1))
            .unwrap();
        registry
            .register(
                Box::new(ReadTextFileTool::new("/tmp", 1024)),
                ToolPolicy::auto(1),
            )
            .unwrap();
        let empty = CapabilitySet::new();
        assert_eq!(registry.specs_for(&empty).len(), 1);
        let mut with_read = CapabilitySet::new();
        with_read.allow(Capability::FsRead(PathBuf::from("/tmp")));
        assert_eq!(registry.specs_for(&with_read).len(), 2);
    }

    #[test]
    fn denies_read_outside_the_root() {
        let root = std::env::temp_dir();
        let mut registry = ToolRegistry::new();
        registry
            .register(
                Box::new(ReadTextFileTool::new(&root, 1024)),
                ToolPolicy::auto(1),
            )
            .unwrap();
        let mut capabilities = CapabilitySet::new();
        capabilities.allow(Capability::FsRead(root.clone()));
        let approved = [];
        let ctx = context(&capabilities, &approved);
        let result = registry.call(
            "read_text_file",
            &parse(r#"{"path":"/etc/passwd"}"#).unwrap(),
            &ctx,
        );
        assert!(result.is_err());
    }

    #[test]
    fn requires_approval_when_policy_asks() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Box::new(EchoTool), ToolPolicy::ask(1))
            .unwrap();
        let capabilities = CapabilitySet::new();
        let no_approval = [];
        let ctx = context(&capabilities, &no_approval);
        assert!(
            registry
                .call("echo", &parse(r#"{"text":"hi"}"#).unwrap(), &ctx)
                .is_err()
        );
        let approved = ["echo".to_string()];
        let ctx = context(&capabilities, &approved);
        assert!(
            registry
                .call("echo", &parse(r#"{"text":"hi"}"#).unwrap(), &ctx)
                .is_ok()
        );
    }

    #[test]
    fn enforces_per_tool_call_limits() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Box::new(WordCountTool), ToolPolicy::auto(1))
            .unwrap();
        let capabilities = CapabilitySet::new();
        let approved = [];
        let ctx = context(&capabilities, &approved);
        let args = parse(r#"{"text":"one two"}"#).unwrap();
        assert!(registry.call("word_count", &args, &ctx).is_ok());
        assert!(registry.call("word_count", &args, &ctx).is_err());
        assert_eq!(registry.calls("word_count"), 1);
    }
}
