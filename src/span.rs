//! Source spans and line/column mapping.

/// A half-open byte range in one source file.
///
/// `file` indexes a [`SourceRegistry`]. Single-source compilation leaves it at
/// zero, so existing callers that only know byte offsets keep working.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub file: u32,
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            file: 0,
            start: start as u32,
            end: end as u32,
        }
    }

    pub fn in_file(file: u32, start: usize, end: usize) -> Self {
        Self {
            file,
            start: start as u32,
            end: end as u32,
        }
    }

    pub fn point(offset: usize) -> Self {
        Self::new(offset, offset)
    }

    pub fn point_in_file(file: u32, offset: usize) -> Self {
        Self::in_file(file, offset, offset)
    }

    /// A sub-span of this span, preserving the source file.
    pub fn sub(self, start: usize, end: usize) -> Self {
        Self::in_file(self.file, start, end)
    }

    pub fn merge(self, other: Span) -> Span {
        Span {
            file: self.file,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    pub fn is_empty(self) -> bool {
        self.start >= self.end
    }

    /// Clamp the span to a source length so rendering can never panic.
    pub fn clamp(self, len: usize) -> Span {
        let len = len as u32;
        Span {
            file: self.file,
            start: self.start.min(len),
            end: self.end.min(len).max(self.start.min(len)),
        }
    }
}

/// One named source file loaded by the module loader.
#[derive(Clone, Debug)]
pub struct SourceFile {
    pub name: String,
    pub source: String,
}

/// Ordered source files addressed by [`Span::file`].
#[derive(Clone, Debug, Default)]
pub struct SourceRegistry {
    files: Vec<SourceFile>,
}

impl SourceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, name: impl Into<String>, source: impl Into<String>) -> u32 {
        let id = self.files.len() as u32;
        self.files.push(SourceFile {
            name: name.into(),
            source: source.into(),
        });
        id
    }

    pub fn get(&self, file: u32) -> Option<&SourceFile> {
        self.files.get(file as usize)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &SourceFile> {
        self.files.iter()
    }
}

/// Maps byte offsets to 1-based line and character columns.
pub struct SourceMap<'a> {
    name: &'a str,
    source: &'a str,
    line_starts: Vec<u32>,
}

impl<'a> SourceMap<'a> {
    pub fn new(name: &'a str, source: &'a str) -> Self {
        let mut line_starts = vec![0u32];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self {
            name,
            source,
            line_starts,
        }
    }

    pub fn name(&self) -> &str {
        self.name
    }

    pub fn source(&self) -> &str {
        self.source
    }

    /// Returns (1-based line, 1-based character column).
    pub fn line_col(&self, offset: u32) -> (usize, usize) {
        let offset = offset.min(self.source.len() as u32);
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(0) => 0,
            Err(i) => i - 1,
        };
        let start = self.line_starts[line_idx] as usize;
        // A span may come from a plugin, a stale cache, or a future tool. Never
        // slice through a UTF-8 code point just because the offset is invalid.
        let mut end = offset as usize;
        while end > start && !self.source.is_char_boundary(end) {
            end -= 1;
        }
        let col = self.source[start..end].chars().count() + 1;
        (line_idx + 1, col)
    }

    /// Returns the text of a 1-based line without the trailing newline.
    pub fn line_text(&self, line: usize) -> &str {
        if line == 0 || line > self.line_starts.len() {
            return "";
        }
        let start = self.line_starts[line - 1] as usize;
        let end = self
            .line_starts
            .get(line)
            .map(|n| *n as usize)
            .unwrap_or(self.source.len());
        let text = &self.source[start..end];
        text.trim_end_matches('\n').trim_end_matches('\r')
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }
}

/// Expands tabs to four spaces for diagnostic alignment.
pub fn expand_tabs(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch == '\t' {
            out.push_str("    ");
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_and_column_are_one_based() {
        let src = "abc\ndef\n";
        let map = SourceMap::new("t.vtx", src);
        assert_eq!(map.line_col(0), (1, 1));
        assert_eq!(map.line_col(2), (1, 3));
        assert_eq!(map.line_col(4), (2, 1));
        assert_eq!(map.line_col(6), (2, 3));
    }

    #[test]
    fn unicode_columns_count_characters() {
        let src = "héllo\n";
        let map = SourceMap::new("t.vtx", src);
        // "h" is 1 byte, "é" is 2 bytes; byte 3 is the second "l".
        assert_eq!(map.line_col(3), (1, 3));
    }

    #[test]
    fn clamping_never_panics() {
        let src = "x";
        let map = SourceMap::new("t.vtx", src);
        assert_eq!(map.line_col(999), (1, 2));
        assert_eq!(Span::new(0, 999).clamp(1), Span::new(0, 1));
    }

    #[test]
    fn offsets_inside_multibyte_characters_are_safe() {
        let src = "aé\n";
        let map = SourceMap::new("t.vtx", src);
        // Byte 2 is the second byte of `é`, not a character boundary.
        assert_eq!(map.line_col(2), (1, 2));
        assert_eq!(map.line_col(3), (1, 3));
    }
}
