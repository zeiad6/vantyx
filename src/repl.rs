//! Interactive REPL input: line editing, history, and completion.
//!
//! The compiler core has no third-party dependencies. On a terminal this
//! module drives a small raw-mode editor through the system `stty` utility;
//! when `stty` is unavailable (or input is not a terminal) it falls back to
//! canonical line input. No `unsafe` code is used.
//!
//! History is stored one entry per line. Multi-line blocks are kept for the
//! current session only; recalling them would require a multi-line editor.
//! The history file is created with mode 0600 on Unix because a REPL entry can
//! contain secrets.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Maximum number of history entries kept in memory and on disk.
pub const MAX_HISTORY: usize = 1000;

/// Keywords and built-in type names offered by completion.
const KEYWORDS: &[&str] = &[
    "fn", "let", "var", "if", "elif", "else", "while", "for", "in", "return", "break",
    "continue", "struct", "enum", "match", "true", "false", "and", "or", "not", "Int", "Float",
    "Bool", "Str", "Unit", "Any", "List", "Map",
];

#[derive(Debug, PartialEq, Eq)]
pub enum ReadOutcome {
    Line(String),
    Eof,
    Interrupted,
}

/// Resolves the history file. `AUREN_HISTORY` overrides the default; the
/// values `off`, `0`, and the empty string disable persistence.
pub fn default_history_path() -> Option<PathBuf> {
    if let Some(value) = env::var_os("AUREN_HISTORY") {
        let value = value.to_string_lossy();
        if value.is_empty() || value == "off" || value == "0" {
            return None;
        }
        return Some(PathBuf::from(value.into_owned()));
    }
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(|home| PathBuf::from(home).join(".auren_history"))
}

pub struct Editor {
    history: Vec<String>,
    names: Vec<String>,
    history_path: Option<PathBuf>,
    history_index: Option<usize>,
    draft: Vec<char>,
}

impl Editor {
    pub fn new(history_path: Option<PathBuf>) -> Self {
        let history = history_path
            .as_deref()
            .map(load_history)
            .unwrap_or_default();
        Self {
            history,
            names: Vec::new(),
            history_path,
            history_index: None,
            draft: Vec::new(),
        }
    }

    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Updates the names offered by completion. Called after an entry runs.
    pub fn set_names(&mut self, names: Vec<String>) {
        self.names = names;
        self.names.sort();
        self.names.dedup();
    }

    /// Records a submitted entry. Empty, multi-line, and consecutive duplicate
    /// entries are ignored.
    pub fn add_history(&mut self, entry: &str) {
        let entry = entry.trim_end();
        if entry.is_empty() || entry.contains('\n') {
            return;
        }
        if self.history.last().is_some_and(|last| last == entry) {
            return;
        }
        self.history.push(entry.to_string());
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        }
        let Some(path) = &self.history_path else {
            return;
        };
        if self.history.len() >= MAX_HISTORY {
            save_history(path, &self.history);
        } else {
            append_history(path, entry);
        }
    }

    /// Reads one logical line. The returned string has no trailing newline.
    pub fn read_line(
        &mut self,
        prompt: &str,
        out: &mut dyn Write,
    ) -> io::Result<ReadOutcome> {
        if !io::stdin().is_terminal() {
            return self.read_line_canonical(prompt, out);
        }
        match RawMode::enter() {
            Some(raw) => self.read_line_raw(prompt, out, raw),
            None => self.read_line_canonical(prompt, out),
        }
    }

    fn read_line_canonical(
        &mut self,
        prompt: &str,
        out: &mut dyn Write,
    ) -> io::Result<ReadOutcome> {
        write!(out, "{prompt}")?;
        out.flush()?;
        let stdin = io::stdin();
        let mut input = stdin.lock();
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Ok(ReadOutcome::Eof);
        }
        Ok(ReadOutcome::Line(
            line.trim_end_matches(['\r', '\n']).to_string(),
        ))
    }

    fn read_line_raw(
        &mut self,
        prompt: &str,
        out: &mut dyn Write,
        _raw: RawMode,
    ) -> io::Result<ReadOutcome> {
        let stdin = io::stdin();
        let mut input = stdin.lock();
        let mut buffer: Vec<char> = Vec::new();
        let mut cursor = 0usize;
        self.history_index = None;
        self.draft.clear();
        repaint(prompt, &buffer, cursor, out)?;
        loop {
            let Some(byte) = read_byte(&mut input)? else {
                out.write_all(b"\r\n")?;
                out.flush()?;
                return Ok(ReadOutcome::Eof);
            };
            match byte {
                b'\r' | b'\n' => {
                    out.write_all(b"\r\n")?;
                    out.flush()?;
                    return Ok(ReadOutcome::Line(buffer.iter().collect()));
                }
                0x03 => {
                    out.write_all(b"^C\r\n")?;
                    out.flush()?;
                    return Ok(ReadOutcome::Interrupted);
                }
                0x04 => {
                    if buffer.is_empty() {
                        out.write_all(b"\r\n")?;
                        out.flush()?;
                        return Ok(ReadOutcome::Eof);
                    }
                    if cursor < buffer.len() {
                        buffer.remove(cursor);
                    }
                }
                0x7f | 0x08 => {
                    if cursor > 0 {
                        cursor -= 1;
                        buffer.remove(cursor);
                    }
                }
                b'\t' => {
                    self.complete(&mut buffer, &mut cursor, out)?;
                }
                0x01 => cursor = 0,
                0x05 => cursor = buffer.len(),
                0x0b => buffer.truncate(cursor),
                0x15 => {
                    buffer.drain(..cursor);
                    cursor = 0;
                }
                0x17 => {
                    let mut start = cursor;
                    while start > 0 && buffer[start - 1].is_whitespace() {
                        start -= 1;
                    }
                    while start > 0 && !buffer[start - 1].is_whitespace() {
                        start -= 1;
                    }
                    buffer.drain(start..cursor);
                    cursor = start;
                }
                0x0c => {
                    out.write_all(b"\x1b[H\x1b[2J")?;
                }
                0x1b => {
                    self.handle_escape(&mut input, &mut buffer, &mut cursor)?;
                }
                0x00..=0x1f => {}
                _ => {
                    let ch = read_char(byte, &mut input)?;
                    buffer.insert(cursor, ch);
                    cursor += 1;
                }
            }
            self.history_index = None;
            repaint(prompt, &buffer, cursor, out)?;
        }
    }

    fn handle_escape(
        &mut self,
        input: &mut impl Read,
        buffer: &mut Vec<char>,
        cursor: &mut usize,
    ) -> io::Result<()> {
        let Some(prefix) = read_byte(input)? else {
            return Ok(());
        };
        if prefix != b'[' && prefix != b'O' {
            return Ok(());
        }
        let Some(code) = read_byte(input)? else {
            return Ok(());
        };
        let mut code = code;
        if !(0x40..=0x7e).contains(&code) {
            loop {
                let Some(next) = read_byte(input)? else {
                    return Ok(());
                };
                if (0x40..=0x7e).contains(&next) {
                    code = next;
                    break;
                }
            }
        }
        match code {
            b'A' => self.history_up(buffer, cursor),
            b'B' => self.history_down(buffer, cursor),
            b'C' => {
                if *cursor < buffer.len() {
                    *cursor += 1;
                }
            }
            b'D' => {
                if *cursor > 0 {
                    *cursor -= 1;
                }
            }
            b'H' => *cursor = 0,
            b'F' => *cursor = buffer.len(),
            b'~' if *cursor < buffer.len() => {
                buffer.remove(*cursor);
            }
            _ => {}
        }
        Ok(())
    }

    fn history_up(&mut self, buffer: &mut Vec<char>, cursor: &mut usize) {
        if self.history.is_empty() {
            return;
        }
        match self.history_index {
            None => {
                self.draft = buffer.clone();
                self.history_index = Some(self.history.len() - 1);
            }
            Some(0) => {}
            Some(index) => self.history_index = Some(index - 1),
        }
        if let Some(index) = self.history_index {
            *buffer = self.history[index].chars().collect();
            *cursor = buffer.len();
        }
    }

    fn history_down(&mut self, buffer: &mut Vec<char>, cursor: &mut usize) {
        let Some(index) = self.history_index else {
            return;
        };
        if index + 1 < self.history.len() {
            self.history_index = Some(index + 1);
            *buffer = self.history[index + 1].chars().collect();
        } else {
            self.history_index = None;
            *buffer = std::mem::take(&mut self.draft);
        }
        *cursor = buffer.len();
    }

    fn complete(
        &mut self,
        buffer: &mut Vec<char>,
        cursor: &mut usize,
        out: &mut dyn Write,
    ) -> io::Result<()> {
        let start = token_start(buffer, *cursor);
        let prefix: String = buffer[start..*cursor].iter().collect();
        if prefix.is_empty() {
            return Ok(());
        }
        let candidates = self.candidates();
        let matches: Vec<&String> = candidates
            .iter()
            .filter(|candidate| candidate.starts_with(&prefix))
            .collect();
        if matches.is_empty() {
            return Ok(());
        }
        let completion = if matches.len() == 1 {
            matches[0].clone()
        } else {
            longest_common_prefix(&matches)
        };
        if completion.chars().count() > prefix.chars().count() {
            buffer.splice(start..*cursor, completion.chars());
            *cursor = start + completion.chars().count();
            return Ok(());
        }
        if matches.len() > 1 {
            out.write_all(b"\r\n")?;
            for chunk in matches.chunks(4) {
                let line = chunk
                    .iter()
                    .map(|name| name.as_str())
                    .collect::<Vec<_>>()
                    .join("  ");
                writeln!(out, "{line}")?;
            }
        }
        Ok(())
    }

    fn candidates(&self) -> Vec<String> {
        let mut names: Vec<String> = KEYWORDS
            .iter()
            .map(|keyword| (*keyword).to_string())
            .collect();
        names.extend(
            auren::checker::BUILTINS
                .iter()
                .map(|builtin| (*builtin).to_string()),
        );
        names.extend(self.names.iter().cloned());
        names.sort();
        names.dedup();
        names
    }
}

fn token_start(buffer: &[char], cursor: usize) -> usize {
    let mut start = cursor;
    while start > 0 && (buffer[start - 1].is_alphanumeric() || buffer[start - 1] == '_') {
        start -= 1;
    }
    start
}

fn longest_common_prefix(values: &[&String]) -> String {
    let Some(first) = values.first() else {
        return String::new();
    };
    let first: Vec<char> = first.chars().collect();
    let mut length = first.len();
    for value in &values[1..] {
        let other: Vec<char> = value.chars().collect();
        length = length.min(other.len());
        for (index, (left, right)) in first.iter().take(length).zip(other.iter()).enumerate() {
            if left != right {
                length = index;
                break;
            }
        }
    }
    first[..length].iter().collect()
}

fn repaint(
    prompt: &str,
    buffer: &[char],
    cursor: usize,
    out: &mut dyn Write,
) -> io::Result<()> {
    let text: String = buffer.iter().collect();
    write!(out, "\r\x1b[2K{prompt}{text}")?;
    let tail = buffer.len().saturating_sub(cursor);
    if tail > 0 {
        write!(out, "\x1b[{tail}D")?;
    }
    out.flush()
}

fn read_byte(input: &mut impl Read) -> io::Result<Option<u8>> {
    let mut byte = [0u8; 1];
    loop {
        match input.read(&mut byte) {
            Ok(0) => return Ok(None),
            Ok(_) => return Ok(Some(byte[0])),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
}

fn read_char(first: u8, input: &mut impl Read) -> io::Result<char> {
    let width = match first {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => return Ok('\u{fffd}'),
    };
    let mut bytes = vec![first];
    for _ in 1..width {
        let Some(next) = read_byte(input)? else {
            return Ok('\u{fffd}');
        };
        bytes.push(next);
    }
    Ok(String::from_utf8_lossy(&bytes)
        .chars()
        .next()
        .unwrap_or('\u{fffd}'))
}

struct RawMode {
    stty: PathBuf,
    saved: String,
}

impl RawMode {
    fn enter() -> Option<Self> {
        let stty = find_stty()?;
        let saved = Command::new(&stty)
            .arg("-g")
            .stdin(Stdio::inherit())
            .output()
            .ok()?;
        if !saved.status.success() {
            return None;
        }
        let saved = String::from_utf8(saved.stdout).ok()?.trim().to_string();
        if saved.is_empty() {
            return None;
        }
        let status = Command::new(&stty)
            .args(["raw", "-echo"])
            .stdin(Stdio::inherit())
            .status()
            .ok()?;
        if !status.success() {
            return None;
        }
        Some(Self { stty, saved })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = Command::new(&self.stty).arg(&self.saved).status();
    }
}

fn find_stty() -> Option<PathBuf> {
    ["/bin/stty", "/usr/bin/stty"]
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
}

fn load_history(path: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut entries: Vec<String> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect();
    if entries.len() > MAX_HISTORY {
        let excess = entries.len() - MAX_HISTORY;
        entries.drain(..excess);
    }
    entries
}

fn append_history(path: &Path, entry: &str) {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let Ok(mut file) = options.open(path) else {
        return;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    let _ = writeln!(file, "{entry}");
}

fn save_history(path: &Path, entries: &[String]) {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let Ok(mut file) = options.open(path) else {
        return;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    for entry in entries {
        let _ = writeln!(file, "{entry}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_text(line: &str, names: &[&str]) -> (String, usize, Vec<String>) {
        let mut editor = Editor::new(None);
        editor.set_names(names.iter().map(|name| (*name).to_string()).collect());
        let mut buffer: Vec<char> = line.chars().collect();
        let mut cursor = buffer.len();
        let mut out = Vec::new();
        editor
            .complete(&mut buffer, &mut cursor, &mut out)
            .expect("completion");
        (
            buffer.into_iter().collect(),
            cursor,
            String::from_utf8(out)
                .expect("utf-8 suggestions")
                .lines()
                .flat_map(|line| line.split_whitespace())
                .map(str::to_string)
                .collect(),
        )
    }

    #[test]
    fn completion_covers_keywords_builtins_and_names() {
        let (line, cursor, _) = complete_text("whi", &[]);
        assert_eq!(line, "while");
        assert_eq!(cursor, 5);
        let (line, _, _) = complete_text("pri", &[]);
        assert_eq!(line, "print");
        let (line, _, _) = complete_text("alp", &["alpha"]);
        assert_eq!(line, "alpha");
    }

    #[test]
    fn completion_extends_to_the_longest_common_prefix() {
        let (line, cursor, _) = complete_text("cou", &["counter", "count"]);
        assert_eq!(line, "count");
        assert_eq!(cursor, 5);
        let (_, _, suggestions) = complete_text("count", &["counter", "count"]);
        assert!(suggestions.contains(&"counter".to_string()));
        assert!(suggestions.contains(&"count".to_string()));
    }

    #[test]
    fn completion_leaves_unknown_prefixes_untouched() {
        let (line, cursor, suggestions) = complete_text("zzz", &["alpha"]);
        assert_eq!(line, "zzz");
        assert_eq!(cursor, 3);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn completion_only_replaces_the_current_token() {
        let (line, _, _) = complete_text("let x = pri", &[]);
        assert_eq!(line, "let x = print");
    }

    #[test]
    fn history_deduplicates_and_skips_multiline_entries() {
        let mut editor = Editor::new(None);
        editor.add_history("let x = 1");
        editor.add_history("let x = 1");
        editor.add_history("if true:\n    print(1)");
        assert_eq!(editor.history(), ["let x = 1"]);
    }

    #[test]
    fn history_file_round_trips_and_is_private() {
        let path = env::temp_dir().join(format!("auren-history-unit-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        let mut editor = Editor::new(Some(path.clone()));
        editor.add_history("let value = 41");
        editor.add_history("print(value + 1)");
        drop(editor);

        let reloaded = Editor::new(Some(path.clone()));
        assert_eq!(
            reloaded.history(),
            ["let value = 41", "print(value + 1)"]
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).expect("history metadata").permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn history_file_is_capped() {
        let path = env::temp_dir().join(format!("auren-history-cap-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        let mut editor = Editor::new(Some(path.clone()));
        for index in 0..=MAX_HISTORY {
            editor.add_history(&format!("let value{index} = {index}"));
        }
        assert_eq!(editor.history().len(), MAX_HISTORY);
        assert_eq!(editor.history()[0], "let value1 = 1");
        drop(editor);

        let reloaded = Editor::new(Some(path.clone()));
        assert_eq!(reloaded.history().len(), MAX_HISTORY);
        let _ = fs::remove_file(&path);
    }
}
