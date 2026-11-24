//! Embedding vectors, similarity, and a deterministic in-memory index.

use super::json::Json;
use super::provider::{AI_E_INVALID_REQUEST, AiError};
use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq)]
pub struct Vector {
    values: Vec<f32>,
}

impl Vector {
    pub fn new(values: Vec<f32>) -> Result<Self, AiError> {
        if values.is_empty() {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                "embedding vectors must not be empty",
            ));
        }
        if values.iter().any(|value| !value.is_finite()) {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                "embedding vectors must contain only finite values",
            ));
        }
        Ok(Self { values })
    }

    pub fn zeros(dimension: usize) -> Result<Self, AiError> {
        Self::new(vec![0.0; dimension])
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn values(&self) -> &[f32] {
        &self.values
    }

    pub fn dot(&self, other: &Self) -> Result<f32, AiError> {
        self.ensure_same_dimension(other)?;
        Ok(self
            .values
            .iter()
            .zip(&other.values)
            .map(|(left, right)| left * right)
            .sum())
    }

    pub fn norm(&self) -> f32 {
        self.values
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt()
    }

    pub fn cosine_similarity(&self, other: &Self) -> Result<f32, AiError> {
        let denominator = self.norm() * other.norm();
        if denominator == 0.0 {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                "cosine similarity is undefined for a zero vector",
            ));
        }
        Ok(self.dot(other)? / denominator)
    }

    pub fn euclidean_distance(&self, other: &Self) -> Result<f32, AiError> {
        self.ensure_same_dimension(other)?;
        Ok(self
            .values
            .iter()
            .zip(&other.values)
            .map(|(left, right)| {
                let difference = left - right;
                difference * difference
            })
            .sum::<f32>()
            .sqrt())
    }

    pub fn normalized(&self) -> Result<Self, AiError> {
        let norm = self.norm();
        if norm == 0.0 {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                "cannot normalize a zero vector",
            ));
        }
        Ok(Self {
            values: self.values.iter().map(|value| value / norm).collect(),
        })
    }

    fn ensure_same_dimension(&self, other: &Self) -> Result<(), AiError> {
        if self.values.len() != other.values.len() {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "embedding dimension mismatch: {} vs {}",
                    self.values.len(),
                    other.values.len()
                ),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VectorEntry {
    pub id: String,
    pub vector: Vector,
    pub metadata: Json,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchHit {
    pub id: String,
    pub score: f32,
    pub metadata: Json,
}

/// A deterministic linear index. It is honest about its O(n) search cost.
#[derive(Clone, Debug, Default)]
pub struct VectorIndex {
    entries: Vec<VectorEntry>,
}

impl VectorIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Inserts or replaces an entry with the same id. Returns the previous
    /// entry when one existed.
    pub fn insert(&mut self, entry: VectorEntry) -> Option<VectorEntry> {
        if let Some(index) = self.entries.iter().position(|item| item.id == entry.id) {
            return Some(std::mem::replace(&mut self.entries[index], entry));
        }
        self.entries.push(entry);
        None
    }

    pub fn remove(&mut self, id: &str) -> Option<VectorEntry> {
        let index = self.entries.iter().position(|entry| entry.id == id)?;
        Some(self.entries.remove(index))
    }

    pub fn get(&self, id: &str) -> Option<&VectorEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub fn search(&self, query: &Vector, limit: usize) -> Result<Vec<SearchHit>, AiError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let mut hits = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            let score = query.cosine_similarity(&entry.vector)?;
            hits.push(SearchHit {
                id: entry.id.clone(),
                score,
                metadata: entry.metadata.clone(),
            });
        }
        hits.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.id.cmp(&right.id))
        });
        hits.truncate(limit);
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vector(values: &[f32]) -> Vector {
        Vector::new(values.to_vec()).unwrap()
    }

    #[test]
    fn computes_dot_and_cosine() {
        let left = vector(&[1.0, 0.0]);
        let right = vector(&[0.0, 2.0]);
        assert_eq!(left.dot(&right).unwrap(), 0.0);
        assert_eq!(left.cosine_similarity(&right).unwrap(), 0.0);
        assert_eq!(left.cosine_similarity(&left).unwrap(), 1.0);
    }

    #[test]
    fn rejects_dimension_mismatch_and_zero_vectors() {
        assert!(
            vector(&[1.0])
                .cosine_similarity(&vector(&[1.0, 2.0]))
                .is_err()
        );
        assert!(
            vector(&[0.0, 0.0])
                .cosine_similarity(&vector(&[1.0, 0.0]))
                .is_err()
        );
    }

    #[test]
    fn normalizes_vectors() {
        let normalized = vector(&[3.0, 4.0]).normalized().unwrap();
        assert!((normalized.norm() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn searches_in_score_order() {
        let mut index = VectorIndex::new();
        index.insert(VectorEntry {
            id: "close".into(),
            vector: vector(&[1.0, 0.0]),
            metadata: Json::Null,
        });
        index.insert(VectorEntry {
            id: "far".into(),
            vector: vector(&[0.0, 1.0]),
            metadata: Json::Null,
        });
        let hits = index.search(&vector(&[1.0, 0.1]), 2).unwrap();
        assert_eq!(hits[0].id, "close");
        assert_eq!(hits.len(), 2);
        assert_eq!(index.search(&vector(&[1.0, 0.0]), 0).unwrap().len(), 0);
    }

    #[test]
    fn replaces_and_removes_entries() {
        let mut index = VectorIndex::new();
        let entry = VectorEntry {
            id: "a".into(),
            vector: vector(&[1.0]),
            metadata: Json::Null,
        };
        assert!(index.insert(entry.clone()).is_none());
        assert!(index.insert(entry).is_some());
        assert_eq!(index.len(), 1);
        assert!(index.remove("a").is_some());
        assert!(index.is_empty());
    }
}
