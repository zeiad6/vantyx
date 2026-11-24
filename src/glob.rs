//! Lexical glob expansion (ADR-0033).
//!
//! Patterns are split on `/`; `*` matches zero or more Unicode scalar values
//! and `?` matches one, character classes use `[a-z]` / `[!a-z]`, and a
//! complete `**` component recurses through real directories without
//! following symbolic links. Results are sorted by UTF-8 bytes and
//! deduplicated. Filesystem failures are fatal (`R0015`); malformed patterns
//! are `R0016`. There is no sandbox.

use std::fs;
use std::io::{self, ErrorKind};

/// Maximum number of candidate paths produced before deduplication.
pub const MAX_RESULTS: usize = 100_000;
/// Maximum number of `**` directory levels below the pattern base.
pub const MAX_DEPTH: usize = 128;
/// Maximum number of pattern components, bounding parser and walk recursion.
pub const MAX_COMPONENTS: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobErrorKind {
    /// The pattern itself is malformed (`R0016`).
    Invalid,
    /// The filesystem could not be inspected, or a limit was reached
    /// (`R0015`).
    Io,
}

#[derive(Debug, Clone)]
pub struct GlobError {
    pub kind: GlobErrorKind,
    pub message: String,
}

impl GlobError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            kind: GlobErrorKind::Invalid,
            message: message.into(),
        }
    }

    fn io(message: impl Into<String>) -> Self {
        Self {
            kind: GlobErrorKind::Io,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
enum Token {
    Literal(char),
    Star,
    Question,
    Class(CharClass),
}

#[derive(Debug, Clone)]
struct CharClass {
    negated: bool,
    ranges: Vec<(char, char)>,
}

impl CharClass {
    fn matches(&self, character: char) -> bool {
        let hit = self
            .ranges
            .iter()
            .any(|(first, last)| *first <= character && character <= *last);
        if self.negated { !hit } else { hit }
    }
}

#[derive(Debug)]
struct Component {
    text: String,
    tokens: Vec<Token>,
    has_meta: bool,
    allows_hidden: bool,
}

impl Component {
    fn new(text: String, tokens: Vec<Token>) -> Self {
        let has_meta = tokens
            .iter()
            .any(|token| !matches!(token, Token::Literal(_)));
        let allows_hidden = match tokens.first() {
            Some(Token::Literal('.')) => true,
            Some(Token::Class(class)) => !class.negated && class.matches('.'),
            _ => false,
        };
        Self {
            text,
            tokens,
            has_meta,
            allows_hidden,
        }
    }

    fn is_double_star(&self) -> bool {
        matches!(self.tokens.as_slice(), [Token::Star, Token::Star])
    }

    fn matches(&self, name: &str) -> bool {
        let characters: Vec<char> = name.chars().collect();
        let mut token_index = 0usize;
        let mut name_index = 0usize;
        let mut star: Option<(usize, usize)> = None;
        while name_index < characters.len() {
            if token_index < self.tokens.len() {
                match &self.tokens[token_index] {
                    Token::Star => {
                        token_index += 1;
                        star = Some((token_index, name_index));
                        continue;
                    }
                    Token::Question => {
                        token_index += 1;
                        name_index += 1;
                        continue;
                    }
                    Token::Class(class) if class.matches(characters[name_index]) => {
                        token_index += 1;
                        name_index += 1;
                        continue;
                    }
                    Token::Literal(expected) if *expected == characters[name_index] => {
                        token_index += 1;
                        name_index += 1;
                        continue;
                    }
                    _ => {}
                }
            }
            if let Some((star_token, star_name)) = star {
                token_index = star_token;
                name_index = star_name + 1;
                star = Some((star_token, star_name + 1));
            } else {
                return false;
            }
        }
        while token_index < self.tokens.len() && matches!(self.tokens[token_index], Token::Star) {
            token_index += 1;
        }
        token_index == self.tokens.len()
    }
}

#[derive(Debug)]
struct Pattern {
    absolute: bool,
    components: Vec<Component>,
}

#[derive(Debug, Clone, Copy)]
struct Limits {
    max_results: usize,
    max_depth: usize,
}

const DEFAULT_LIMITS: Limits = Limits {
    max_results: MAX_RESULTS,
    max_depth: MAX_DEPTH,
};

/// Expands `pattern` against the process working directory.
pub fn glob(pattern: &str) -> Result<Vec<String>, GlobError> {
    glob_with_limits(pattern, DEFAULT_LIMITS)
}

fn glob_with_limits(pattern: &str, limits: Limits) -> Result<Vec<String>, GlobError> {
    if pattern.contains('\0') {
        return Err(GlobError::io("glob pattern contains a NUL byte"));
    }
    let parsed = parse_pattern(pattern)?;
    let mut results = Vec::new();
    if parsed.components.is_empty() {
        if parsed.absolute && fs::symlink_metadata("/").is_ok() {
            push_result(&mut results, "/".to_string(), limits)?;
        }
    } else {
        let base = if parsed.absolute { "/" } else { "" };
        walk(&parsed.components, 0, base, 0, limits, &mut results)?;
    }
    results.sort();
    results.dedup();
    Ok(results)
}

fn parse_pattern(pattern: &str) -> Result<Pattern, GlobError> {
    if pattern.is_empty() {
        return Err(GlobError::invalid("glob pattern is empty"));
    }
    let absolute = pattern.starts_with('/');
    let mut components = Vec::new();
    for text in pattern.split('/') {
        if text.is_empty() {
            continue;
        }
        let tokens = parse_component(text)
            .map_err(|message| GlobError::invalid(format!("invalid glob pattern: {message}")))?;
        components.push(Component::new(text.to_string(), tokens));
    }
    if components.len() > MAX_COMPONENTS {
        return Err(GlobError::invalid(format!(
            "glob pattern has more than {MAX_COMPONENTS} components"
        )));
    }
    Ok(Pattern {
        absolute,
        components,
    })
}

fn parse_component(text: &str) -> Result<Vec<Token>, String> {
    let characters: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < characters.len() {
        match characters[index] {
            '*' => {
                tokens.push(Token::Star);
                index += 1;
            }
            '?' => {
                tokens.push(Token::Question);
                index += 1;
            }
            '[' => {
                tokens.push(Token::Class(parse_class(&characters, &mut index)?));
            }
            character => {
                tokens.push(Token::Literal(character));
                index += 1;
            }
        }
    }
    Ok(tokens)
}

fn parse_class(characters: &[char], index: &mut usize) -> Result<CharClass, String> {
    debug_assert_eq!(characters[*index], '[');
    *index += 1;
    let negated =
        if *index < characters.len() && (characters[*index] == '!' || characters[*index] == '^') {
            *index += 1;
            true
        } else {
            false
        };
    let mut ranges = Vec::new();
    if *index < characters.len() && characters[*index] == ']' {
        ranges.push((']', ']'));
        *index += 1;
    }
    let mut closed = false;
    while *index < characters.len() {
        if characters[*index] == ']' {
            *index += 1;
            closed = true;
            break;
        }
        let first = characters[*index];
        *index += 1;
        if *index + 1 < characters.len()
            && characters[*index] == '-'
            && characters[*index + 1] != ']'
        {
            let last = characters[*index + 1];
            if last < first {
                return Err(format!("descending character range `{first}-{last}`"));
            }
            ranges.push((first, last));
            *index += 2;
        } else {
            ranges.push((first, first));
        }
    }
    if !closed {
        return Err("unclosed character class".to_string());
    }
    if ranges.is_empty() {
        return Err("empty character class".to_string());
    }
    Ok(CharClass { negated, ranges })
}

fn push_result(results: &mut Vec<String>, value: String, limits: Limits) -> Result<(), GlobError> {
    if results.len() >= limits.max_results {
        return Err(GlobError::io(format!(
            "glob produced more than {} candidate paths",
            limits.max_results
        )));
    }
    results.push(value);
    Ok(())
}

fn join(base: &str, name: &str) -> String {
    if base.is_empty() {
        name.to_string()
    } else if base == "/" {
        format!("/{name}")
    } else {
        format!("{base}/{name}")
    }
}

fn is_absent(error: &io::Error) -> bool {
    matches!(error.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory)
}

/// Reads a directory's UTF-8 entry names, excluding `.` and `..`. `None`
/// means the path is missing, is not a directory, or has disappeared.
fn read_names(path: &str) -> Result<Option<Vec<String>>, GlobError> {
    let target = if path.is_empty() { "." } else { path };
    let metadata = match fs::metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if is_absent(&error) => return Ok(None),
        Err(error) => {
            return Err(GlobError::io(format!("cannot inspect `{target}`: {error}")));
        }
    };
    if !metadata.is_dir() {
        return Ok(None);
    }
    let entries = match fs::read_dir(target) {
        Ok(entries) => entries,
        Err(error) if is_absent(&error) => return Ok(None),
        Err(error) => {
            return Err(GlobError::io(format!(
                "cannot list directory `{target}`: {error}"
            )));
        }
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry
            .map_err(|error| GlobError::io(format!("cannot list directory `{target}`: {error}")))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Err(GlobError::io(format!(
                "directory `{target}` contains a non-UTF-8 entry name"
            )));
        };
        names.push(name.to_string());
    }
    Ok(Some(names))
}

fn is_real_directory(path: &str) -> Result<bool, GlobError> {
    let target = if path.is_empty() { "." } else { path };
    match fs::symlink_metadata(target) {
        Ok(metadata) => Ok(metadata.is_dir() && !metadata.file_type().is_symlink()),
        Err(error) if is_absent(&error) => Ok(false),
        Err(error) => Err(GlobError::io(format!("cannot inspect `{target}`: {error}"))),
    }
}

fn collect_descendants(
    base: &str,
    depth: usize,
    limits: Limits,
    results: &mut Vec<String>,
) -> Result<(), GlobError> {
    if depth > limits.max_depth {
        return Err(GlobError::io(format!(
            "glob exceeded maximum depth of {}",
            limits.max_depth
        )));
    }
    let Some(names) = read_names(base)? else {
        return Ok(());
    };
    for name in names {
        if name.starts_with('.') {
            continue;
        }
        let child = join(base, &name);
        push_result(results, child.clone(), limits)?;
        if is_real_directory(&child)? {
            collect_descendants(&child, depth + 1, limits, results)?;
        }
    }
    Ok(())
}

fn walk(
    components: &[Component],
    index: usize,
    base: &str,
    depth: usize,
    limits: Limits,
    results: &mut Vec<String>,
) -> Result<(), GlobError> {
    if index == components.len() {
        if !base.is_empty() {
            push_result(results, base.to_string(), limits)?;
        }
        return Ok(());
    }
    let component = &components[index];
    let last = index + 1 == components.len();
    if component.is_double_star() {
        if last {
            return collect_descendants(base, depth, limits, results);
        }
        walk(components, index + 1, base, depth, limits, results)?;
        let Some(names) = read_names(base)? else {
            return Ok(());
        };
        for name in names {
            if name.starts_with('.') {
                continue;
            }
            let child = join(base, &name);
            if is_real_directory(&child)? {
                if depth + 1 > limits.max_depth {
                    return Err(GlobError::io(format!(
                        "glob exceeded maximum depth of {}",
                        limits.max_depth
                    )));
                }
                walk(components, index, &child, depth + 1, limits, results)?;
            }
        }
        return Ok(());
    }
    if component.has_meta {
        let Some(names) = read_names(base)? else {
            return Ok(());
        };
        for name in names {
            if name.starts_with('.') && !component.allows_hidden {
                continue;
            }
            if !component.matches(&name) {
                continue;
            }
            let child = join(base, &name);
            if last {
                push_result(results, child, limits)?;
            } else {
                walk(components, index + 1, &child, depth, limits, results)?;
            }
        }
        return Ok(());
    }
    let child = join(base, &component.text);
    if last {
        if fs::symlink_metadata(&child).is_ok() {
            push_result(results, child, limits)?;
        }
    } else {
        walk(components, index + 1, &child, depth, limits, results)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn workspace(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "vantyx-glob-unit-{}-{}-{name}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("create glob workspace");
        directory
    }

    fn parse(text: &str) -> Component {
        let tokens = parse_component(text).expect("valid component");
        Component::new(text.to_string(), tokens)
    }

    fn matches(pattern: &str, name: &str) -> bool {
        parse(pattern).matches(name)
    }

    #[test]
    fn components_match_literals_wildcards_and_classes() {
        assert!(matches("*.txt", "a.txt"));
        assert!(matches("*.txt", "alpha.beta.txt"));
        assert!(!matches("*.txt", "a.vtx"));
        assert!(matches("?", "é"));
        assert!(matches("??", "αβ"));
        assert!(matches("[ab].vtx", "a.vtx"));
        assert!(matches("[a-z].vtx", "q.vtx"));
        assert!(matches("[!a].vtx", "b.vtx"));
        assert!(!matches("[!a].vtx", "a.vtx"));
        assert!(matches("[^α].vtx", "β.vtx"));
        assert!(matches("[]].vtx", "].vtx"));
        assert!(matches("[[]x", "[x"));
        assert!(matches("[*].txt", "*.txt"));
        assert!(matches("[?].txt", "?.txt"));
        assert!(matches("[a-].txt", "-.txt"));
        assert!(matches("[-a].txt", "-.txt"));
        assert!(!matches("a*b", "a"));
        assert!(matches("a*b", "ab"));
        assert!(matches("a*b", "axxb"));
    }

    #[test]
    fn components_apply_hidden_rules() {
        assert!(!parse("*").allows_hidden);
        assert!(!parse("*.txt").allows_hidden);
        assert!(!parse("[!a]*").allows_hidden);
        assert!(parse(".*").allows_hidden);
        assert!(parse("[.]dot").allows_hidden);
        assert!(parse("[.-0]dot").allows_hidden);
        assert!(!parse("a*").allows_hidden);
        assert!(parse("**").is_double_star());
        assert!(!parse("***").is_double_star());
        assert!(!parse("*").is_double_star());
    }

    #[test]
    fn malformed_patterns_are_invalid() {
        for pattern in ["", "[", "[]", "[!]", "[z-a]", "[a--b]"] {
            let error = match glob(pattern) {
                Ok(_) => panic!("pattern {pattern:?} unexpectedly matched"),
                Err(error) => error,
            };
            assert_eq!(error.kind, GlobErrorKind::Invalid, "{pattern:?}");
        }
        let error = glob(&"a/".repeat(MAX_COMPONENTS + 1)).expect_err("too many components");
        assert_eq!(error.kind, GlobErrorKind::Invalid);
    }

    #[test]
    fn literal_probes_do_not_read_directories() {
        let directory = workspace("literal");
        fs::write(directory.join("file.txt"), "x").expect("write file");
        let file = directory.join("file.txt");
        let missing = directory.join("missing");
        assert_eq!(
            glob(file.to_str().expect("utf-8 path")).expect("probe file"),
            [file.to_str().expect("utf-8 path")]
        );
        assert!(
            glob(missing.to_str().expect("utf-8 path"))
                .expect("probe missing")
                .is_empty()
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn glob_expands_a_tree_with_sorted_unique_results() {
        let directory = workspace("tree");
        fs::create_dir_all(directory.join("dir/sub/deep")).expect("create dirs");
        fs::create_dir_all(directory.join(".hidden")).expect("create hidden");
        fs::write(directory.join("a.txt"), "a").expect("write a");
        fs::write(directory.join("b.vtx"), "b").expect("write b");
        fs::write(directory.join("dir/b.txt"), "c").expect("write c");
        fs::write(directory.join("dir/.dot"), "d").expect("write dot");
        fs::write(directory.join("dir/sub/c.vtx"), "e").expect("write e");
        fs::write(directory.join("dir/sub/deep/d.txt"), "f").expect("write f");
        fs::write(directory.join(".hidden/h.txt"), "g").expect("write g");
        let root = directory.to_str().expect("utf-8 path").to_string();
        let expand = |pattern: &str| {
            glob(&format!("{root}/{pattern}"))
                .expect("glob")
                .into_iter()
                .map(|path| path[root.len() + 1..].to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(expand("*.txt"), ["a.txt"]);
        assert_eq!(expand("dir/*"), ["dir/b.txt", "dir/sub"]);
        assert_eq!(
            expand("**/*.txt"),
            ["a.txt", "dir/b.txt", "dir/sub/deep/d.txt"]
        );
        assert_eq!(expand("dir/**/*.vtx"), ["dir/sub/c.vtx"]);
        assert_eq!(
            expand("dir/**"),
            [
                "dir/b.txt",
                "dir/sub",
                "dir/sub/c.vtx",
                "dir/sub/deep",
                "dir/sub/deep/d.txt"
            ]
        );
        assert_eq!(expand("**/.*"), [".hidden", "dir/.dot"]);
        assert_eq!(expand("missing/*"), Vec::<String>::new());
        assert_eq!(expand("missing/**"), Vec::<String>::new());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn limits_are_enforced() {
        let directory = workspace("limits");
        for name in ["a", "b", "c"] {
            fs::write(directory.join(name), "x").expect("write file");
        }
        let pattern = format!("{}/*", directory.display());
        let result = glob_with_limits(
            &pattern,
            Limits {
                max_results: 2,
                max_depth: 4,
            },
        );
        let error = result.expect_err("result cap");
        assert_eq!(error.kind, GlobErrorKind::Io);
        assert!(
            error.message.contains("candidate paths"),
            "{}",
            error.message
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn depth_limit_is_enforced() {
        let directory = workspace("depth");
        let mut path = directory.clone();
        for index in 0..(MAX_DEPTH + 2) {
            path.push(format!("d{index}"));
        }
        fs::create_dir_all(&path).expect("create deep tree");
        let pattern = format!("{}/**", directory.display());
        let error = glob(&pattern).expect_err("depth cap");
        assert_eq!(error.kind, GlobErrorKind::Io);
        assert!(error.message.contains("maximum depth"), "{}", error.message);
        let _ = fs::remove_dir_all(directory);
    }

    #[cfg(unix)]
    #[test]
    fn recursive_wildcards_do_not_follow_symlinks() {
        use std::os::unix::fs::symlink;
        let directory = workspace("symlink");
        fs::create_dir_all(directory.join("real/sub")).expect("create real");
        fs::write(directory.join("real/sub/file.txt"), "x").expect("write file");
        symlink(directory.join("real"), directory.join("link")).expect("create link");
        symlink(directory.join("missing"), directory.join("dangling")).expect("create dangling");
        let root = directory.to_str().expect("utf-8 path").to_string();
        let expand = |pattern: &str| {
            glob(&format!("{root}/{pattern}"))
                .expect("glob")
                .into_iter()
                .map(|path| path[root.len() + 1..].to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(expand("link/**"), ["link/sub", "link/sub/file.txt"]);
        assert_eq!(
            expand("**"),
            ["dangling", "link", "real", "real/sub", "real/sub/file.txt"]
        );
        assert_eq!(expand("dangling"), ["dangling"]);
        assert_eq!(expand("dangling/*"), Vec::<String>::new());
        let _ = fs::remove_dir_all(directory);
    }
}
