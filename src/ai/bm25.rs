//! Deterministic BM25 lexical retrieval. This is keyword search, not semantic
//! search; hybrid callers must label the difference.

use std::cmp::Ordering;
use std::collections::HashMap;

pub const K1: f32 = 1.2;
pub const B: f32 = 0.75;

/// Lowercases and splits on non-alphanumeric characters.
pub fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        if character.is_alphanumeric() {
            current.extend(character.to_lowercase());
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[derive(Clone, Debug)]
struct Document {
    id: String,
    text: String,
    term_frequencies: HashMap<String, usize>,
    length: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bm25Hit {
    pub id: String,
    pub score: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Bm25Index {
    documents: Vec<Document>,
}

impl Bm25Index {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.documents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    pub fn insert(&mut self, id: impl Into<String>, text: impl Into<String>) -> Option<String> {
        let text = text.into();
        let tokens = tokenize(&text);
        let mut term_frequencies = HashMap::new();
        for token in &tokens {
            *term_frequencies.entry(token.clone()).or_insert(0) += 1;
        }
        let document = Document {
            id: id.into(),
            text,
            term_frequencies,
            length: tokens.len(),
        };
        if let Some(index) = self
            .documents
            .iter()
            .position(|existing| existing.id == document.id)
        {
            let previous = std::mem::replace(&mut self.documents[index], document);
            return Some(previous.id);
        }
        self.documents.push(document);
        None
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let Some(index) = self.documents.iter().position(|document| document.id == id) else {
            return false;
        };
        self.documents.remove(index);
        true
    }

    pub fn text(&self, id: &str) -> Option<&str> {
        self.documents
            .iter()
            .find(|document| document.id == id)
            .map(|document| document.text.as_str())
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<Bm25Hit> {
        if limit == 0 || self.documents.is_empty() {
            return Vec::new();
        }
        let mut terms: Vec<String> = Vec::new();
        for term in tokenize(query) {
            if !terms.contains(&term) {
                terms.push(term);
            }
        }
        if terms.is_empty() {
            return Vec::new();
        }

        let document_count = self.documents.len() as f32;
        let average_length = self
            .documents
            .iter()
            .map(|document| document.length)
            .sum::<usize>() as f32
            / document_count;
        let average_length = average_length.max(1.0);

        let mut hits = Vec::with_capacity(self.documents.len());
        for document in &self.documents {
            let mut score = 0.0_f32;
            for term in &terms {
                let Some(&frequency) = document.term_frequencies.get(term) else {
                    continue;
                };
                let document_frequency = self
                    .documents
                    .iter()
                    .filter(|candidate| candidate.term_frequencies.contains_key(term))
                    .count() as f32;
                let idf = (1.0
                    + (document_count - document_frequency + 0.5) / (document_frequency + 0.5))
                    .ln();
                let length_ratio = document.length as f32 / average_length;
                let denominator = frequency as f32 + K1 * (1.0 - B + B * length_ratio);
                score += idf * (frequency as f32 * (K1 + 1.0)) / denominator;
            }
            if score > 0.0 {
                hits.push(Bm25Hit {
                    id: document.id.clone(),
                    score,
                });
            }
        }
        hits.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.id.cmp(&right.id))
        });
        hits.truncate(limit);
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_punctuation_and_unicode() {
        assert_eq!(tokenize("Hello, world! 42"), vec!["hello", "world", "42"]);
        assert_eq!(tokenize("naïve café"), vec!["naïve", "café"]);
    }

    #[test]
    fn ranks_matching_documents_first() {
        let mut index = Bm25Index::new();
        index.insert("rust", "Rust is a systems programming language");
        index.insert("python", "Python is a scripting language");
        index.insert("coffee", "Coffee is a drink");
        let hits = index.search("systems language", 2);
        assert_eq!(hits[0].id, "rust");
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn returns_nothing_for_an_empty_query() {
        let mut index = Bm25Index::new();
        index.insert("a", "some text");
        assert!(index.search("", 5).is_empty());
        assert!(index.search("text", 0).is_empty());
    }

    #[test]
    fn replaces_and_removes_documents() {
        let mut index = Bm25Index::new();
        assert!(index.insert("a", "first").is_none());
        assert!(index.insert("a", "second").is_some());
        assert_eq!(index.text("a"), Some("second"));
        assert!(index.remove("a"));
        assert!(!index.remove("a"));
        assert!(index.is_empty());
    }
}
