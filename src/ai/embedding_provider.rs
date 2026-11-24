//! Deterministic offline embeddings and a bounded embedding cache.
//!
//! [`HashEmbeddingProvider`] is a feature-hashing baseline for tests, examples,
//! and plumbing demos. It is **not** a trained semantic model and must not be
//! described as one. Real semantic embeddings come from an embedding model
//! served by an OpenAI-compatible endpoint (see the adapter in `openai.rs`) or
//! a future in-process model behind an explicit dependency decision.
//!
//! [`CachingEmbeddingProvider`] wraps any provider that advertises embeddings.
//! It is an in-memory FIFO cache with hard entry and byte limits, so a long
//! process cannot grow without bound. Cached vectors never leave the process
//! unless the caller writes them.

use super::budget::Budget;
use super::embedding::Vector;
use super::provider::{
    AI_E_INVALID_REQUEST, AI_E_PROVIDER, AiError, EmbeddingRequest, EmbeddingResponse,
    EmbeddingUsage, MAX_EMBEDDING_DIMENSIONS, Provider, ProviderCapabilities,
};
use super::tokens::estimate_embedding_tokens;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};

pub const DEFAULT_HASH_DIMENSIONS: usize = 64;
pub const DEFAULT_CACHE_ENTRIES: usize = 1024;
pub const DEFAULT_CACHE_BYTES: usize = 4 * 1024 * 1024;

/// Deterministic feature-hash embeddings.
///
/// Text is lowercased, split into alphanumeric tokens, and augmented with
/// character trigrams. Features are signed-hashed into a fixed-size vector and
/// normalized. This gives stable, non-zero vectors for offline tests and
/// similarity plumbing, but it captures lexical overlap only.
pub struct HashEmbeddingProvider {
    id: String,
    dimensions: usize,
    capabilities: ProviderCapabilities,
}

impl HashEmbeddingProvider {
    pub fn new(dimensions: usize) -> Result<Self, AiError> {
        if !(1..=MAX_EMBEDDING_DIMENSIONS).contains(&dimensions) {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "hash embedding dimensions must be between 1 and {MAX_EMBEDDING_DIMENSIONS}"
                ),
            ));
        }
        Ok(Self {
            id: "hash".into(),
            dimensions,
            capabilities: ProviderCapabilities::hash_embedding(),
        })
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn dimensions(&self) -> usize {
        self.dimensions
    }
}

impl Default for HashEmbeddingProvider {
    fn default() -> Self {
        Self {
            id: "hash".into(),
            dimensions: DEFAULT_HASH_DIMENSIONS,
            capabilities: ProviderCapabilities::hash_embedding(),
        }
    }
}

impl Provider for HashEmbeddingProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities
    }

    fn embed(
        &self,
        request: &EmbeddingRequest,
        budget: &mut Budget,
    ) -> Result<EmbeddingResponse, AiError> {
        request.validate()?;
        if let Some(dimensions) = request.dimensions
            && dimensions != self.dimensions
        {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "hash provider is configured for {} dimensions, but the request asked for {dimensions}; omit dimensions or use {}",
                    self.dimensions, self.dimensions
                ),
            ));
        }
        let estimate = u64::try_from(estimate_embedding_tokens(request)).unwrap_or(u64::MAX);
        budget.check_tokens(estimate)?;
        let vectors = request
            .input
            .iter()
            .map(|text| hash_vector(text, self.dimensions))
            .collect();
        let usage = EmbeddingUsage::new(estimate);
        budget.record_tokens(usage.total_tokens)?;
        Ok(EmbeddingResponse {
            model: request.model.clone(),
            vectors,
            usage,
        })
    }
}

fn hash_vector(text: &str, dimensions: usize) -> Vector {
    let mut values = vec![0.0_f32; dimensions];
    let lowered = text.to_lowercase();
    let mut tokens = 0usize;
    for token in lowered.split(|character: char| !character.is_alphanumeric()) {
        if token.is_empty() {
            continue;
        }
        add_feature(&mut values, token, 1.0);
        tokens += 1;
    }
    let characters: Vec<char> = lowered
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    if characters.len() >= 3 {
        for window in characters.windows(3) {
            let gram: String = window.iter().collect();
            add_feature(&mut values, &gram, 0.5);
        }
    }
    if tokens == 0 && characters.is_empty() {
        // Whitespace-only input is rejected by request validation, but keep the
        // helper total for any future caller.
        add_feature(&mut values, text, 1.0);
    }
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm == 0.0 {
        let index = (fnv1a(text.as_bytes()) % dimensions as u64) as usize;
        values[index] = 1.0;
    } else {
        for value in &mut values {
            *value /= norm;
        }
    }
    Vector::new(values).expect("hash vectors are non-empty and finite")
}

fn add_feature(values: &mut [f32], feature: &str, weight: f32) {
    let hash = fnv1a(feature.as_bytes());
    let index = (hash % values.len() as u64) as usize;
    let sign = if hash & (1 << 63) == 0 { 1.0 } else { -1.0 };
    values[index] += sign * weight;
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EmbeddingCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub entries: usize,
    pub bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CacheKey {
    model: String,
    dimensions: Option<usize>,
    text: String,
}

struct CacheEntry {
    vector: Vector,
    bytes: usize,
}

struct CacheState {
    entries: HashMap<CacheKey, CacheEntry>,
    order: VecDeque<CacheKey>,
    bytes: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
    max_entries: usize,
    max_bytes: usize,
}

impl CacheState {
    fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            max_entries: max_entries.max(1),
            max_bytes: max_bytes.max(1),
        }
    }

    fn insert(&mut self, key: CacheKey, vector: Vector) {
        let bytes = cache_entry_bytes(&key, &vector);
        if bytes > self.max_bytes {
            return;
        }
        if let Some(previous) = self.entries.remove(&key) {
            self.bytes = self.bytes.saturating_sub(previous.bytes);
            if let Some(position) = self.order.iter().position(|item| item == &key) {
                self.order.remove(position);
            }
        }
        while self.entries.len() >= self.max_entries
            || self.bytes.saturating_add(bytes) > self.max_bytes
        {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(entry.bytes);
                self.evictions = self.evictions.saturating_add(1);
            }
        }
        self.bytes = self.bytes.saturating_add(bytes);
        self.order.push_back(key.clone());
        self.entries.insert(key, CacheEntry { vector, bytes });
    }

    fn clear(&mut self) {
        let max_entries = self.max_entries;
        let max_bytes = self.max_bytes;
        *self = Self::new(max_entries, max_bytes);
    }
}

fn cache_entry_bytes(key: &CacheKey, vector: &Vector) -> usize {
    key.model
        .len()
        .saturating_add(key.text.len())
        .saturating_add(vector.len().saturating_mul(std::mem::size_of::<f32>()))
        .saturating_add(64)
}

/// An in-memory FIFO cache for embedding vectors.
///
/// The cache key includes the model, requested dimensions, and input text, so
/// changing either model or dimensions cannot return a stale vector. Repeated
/// inputs in one batch are sent to the provider once. A cache hit consumes no
/// provider tokens because no provider call is made.
pub struct CachingEmbeddingProvider<P: Provider> {
    inner: P,
    state: RefCell<CacheState>,
}

impl<P: Provider> CachingEmbeddingProvider<P> {
    pub fn new(inner: P) -> Self {
        Self::with_limits(inner, DEFAULT_CACHE_ENTRIES, DEFAULT_CACHE_BYTES)
    }

    pub fn with_limits(inner: P, max_entries: usize, max_bytes: usize) -> Self {
        Self {
            inner,
            state: RefCell::new(CacheState::new(max_entries, max_bytes)),
        }
    }

    pub fn inner(&self) -> &P {
        &self.inner
    }

    pub fn stats(&self) -> EmbeddingCacheStats {
        let state = self.state.borrow();
        EmbeddingCacheStats {
            hits: state.hits,
            misses: state.misses,
            evictions: state.evictions,
            entries: state.entries.len(),
            bytes: state.bytes,
        }
    }

    /// Empties the cache and resets its counters.
    pub fn clear(&self) {
        self.state.borrow_mut().clear();
    }
}

impl<P: Provider> Provider for CachingEmbeddingProvider<P> {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }

    fn chat(
        &self,
        request: &super::provider::ChatRequest,
        budget: &mut Budget,
    ) -> Result<super::provider::ChatResponse, AiError> {
        self.inner.chat(request, budget)
    }

    fn embed(
        &self,
        request: &EmbeddingRequest,
        budget: &mut Budget,
    ) -> Result<EmbeddingResponse, AiError> {
        request.validate()?;
        if !self.inner.capabilities().embeddings {
            return Err(AiError::unsupported("embeddings", self.inner.id()));
        }

        let mut keys = Vec::new();
        let mut positions: HashMap<CacheKey, Vec<usize>> = HashMap::new();
        for (index, text) in request.input.iter().enumerate() {
            let key = CacheKey {
                model: request.model.clone(),
                dimensions: request.dimensions,
                text: text.clone(),
            };
            if !positions.contains_key(&key) {
                keys.push(key.clone());
            }
            positions.entry(key).or_default().push(index);
        }

        let mut results: Vec<Option<Vector>> = vec![None; request.input.len()];
        let mut missing = Vec::new();
        let mut hits = 0_u64;
        {
            let state = self.state.borrow();
            for key in &keys {
                if let Some(entry) = state.entries.get(key) {
                    for &index in &positions[key] {
                        results[index] = Some(entry.vector.clone());
                    }
                    hits = hits
                        .saturating_add(u64::try_from(positions[key].len()).unwrap_or(u64::MAX));
                } else {
                    missing.push(key.clone());
                }
            }
        }

        let mut usage = EmbeddingUsage::default();
        let mut model = request.model.clone();
        if !missing.is_empty() {
            let mut subrequest = request.clone();
            subrequest.input = missing.iter().map(|key| key.text.clone()).collect();
            let response = self.inner.embed(&subrequest, budget)?;
            if response.vectors.len() != missing.len() {
                return Err(AiError::new(
                    AI_E_PROVIDER,
                    format!(
                        "provider returned {} embeddings for {} uncached inputs",
                        response.vectors.len(),
                        missing.len()
                    ),
                ));
            }
            model = response.model;
            usage = response.usage;
            let mut state = self.state.borrow_mut();
            state.misses = state
                .misses
                .saturating_add(u64::try_from(missing.len()).unwrap_or(u64::MAX));
            for (key, vector) in missing.into_iter().zip(response.vectors) {
                for &index in &positions[&key] {
                    results[index] = Some(vector.clone());
                }
                state.insert(key, vector);
            }
        }
        {
            let mut state = self.state.borrow_mut();
            state.hits = state.hits.saturating_add(hits);
        }

        let vectors = results
            .into_iter()
            .map(|item| {
                item.ok_or_else(|| {
                    AiError::new(
                        AI_E_PROVIDER,
                        "provider did not return a vector for every input",
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(EmbeddingResponse {
            model,
            vectors,
            usage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn request(model: &str, inputs: &[&str]) -> EmbeddingRequest {
        EmbeddingRequest::new(
            model,
            inputs.iter().map(|text| (*text).to_string()).collect(),
        )
    }

    fn vector(values: &[f32]) -> Vector {
        Vector::new(values.to_vec()).unwrap()
    }

    struct CountingProvider {
        inner: HashEmbeddingProvider,
        calls: Cell<usize>,
        inputs: Cell<usize>,
    }

    impl CountingProvider {
        fn new(dimensions: usize) -> Self {
            Self {
                inner: HashEmbeddingProvider::new(dimensions).unwrap(),
                calls: Cell::new(0),
                inputs: Cell::new(0),
            }
        }
    }

    impl Provider for CountingProvider {
        fn id(&self) -> &str {
            "counting"
        }

        fn capabilities(&self) -> ProviderCapabilities {
            self.inner.capabilities()
        }

        fn embed(
            &self,
            request: &EmbeddingRequest,
            budget: &mut Budget,
        ) -> Result<EmbeddingResponse, AiError> {
            self.calls.set(self.calls.get() + 1);
            self.inputs.set(self.inputs.get() + request.input.len());
            self.inner.embed(request, budget)
        }
    }

    #[test]
    fn hash_embeddings_are_deterministic_and_normalized() {
        let provider = HashEmbeddingProvider::new(16).unwrap();
        let mut budget = Budget::default();
        let first = provider
            .embed(&request("m", &["hello world"]), &mut budget)
            .unwrap();
        let second = provider
            .embed(&request("m", &["hello world"]), &mut budget)
            .unwrap();
        assert_eq!(first.vectors, second.vectors);
        assert_eq!(first.vectors[0].len(), 16);
        assert!((first.vectors[0].norm() - 1.0).abs() < 1e-5);
        let other = provider
            .embed(&request("m", &["different text"]), &mut budget)
            .unwrap();
        assert_ne!(first.vectors[0], other.vectors[0]);
    }

    #[test]
    fn hash_embeddings_preserve_input_order() {
        let provider = HashEmbeddingProvider::new(8).unwrap();
        let mut budget = Budget::default();
        let batch = provider
            .embed(&request("m", &["alpha", "beta"]), &mut budget)
            .unwrap();
        let alpha = provider
            .embed(&request("m", &["alpha"]), &mut budget)
            .unwrap();
        let beta = provider
            .embed(&request("m", &["beta"]), &mut budget)
            .unwrap();
        assert_eq!(batch.vectors[0], alpha.vectors[0]);
        assert_eq!(batch.vectors[1], beta.vectors[0]);
    }

    #[test]
    fn hash_embeddings_reject_a_dimension_mismatch() {
        let provider = HashEmbeddingProvider::new(8).unwrap();
        let mut budget = Budget::default();
        let mut request = request("m", &["hello"]);
        request.dimensions = Some(16);
        let error = provider.embed(&request, &mut budget).unwrap_err();
        assert_eq!(error.code, AI_E_INVALID_REQUEST);
        assert!(error.message.contains("8 dimensions"));
    }

    #[test]
    fn hash_embeddings_respect_the_token_budget() {
        let provider = HashEmbeddingProvider::new(8).unwrap();
        let mut budget = Budget::default().with_tokens(1);
        let error = provider
            .embed(&request("m", &["a long embedding input"]), &mut budget)
            .unwrap_err();
        assert_eq!(error.code, super::super::provider::AI_E_BUDGET);
    }

    #[test]
    fn hash_embeddings_handle_punctuation_only_input() {
        let provider = HashEmbeddingProvider::new(4).unwrap();
        let mut budget = Budget::default();
        let response = provider
            .embed(&request("m", &["!!!"]), &mut budget)
            .unwrap();
        assert_eq!(response.vectors[0].len(), 4);
        assert!(response.vectors[0].norm() > 0.0);
    }

    #[test]
    fn cache_serves_repeated_inputs_without_another_provider_call() {
        let provider = CachingEmbeddingProvider::new(CountingProvider::new(8));
        let mut budget = Budget::default();
        let first = provider
            .embed(&request("m", &["alpha", "beta"]), &mut budget)
            .unwrap();
        let second = provider
            .embed(&request("m", &["alpha", "beta"]), &mut budget)
            .unwrap();
        assert_eq!(first.vectors, second.vectors);
        assert_eq!(provider.inner().calls.get(), 1);
        assert_eq!(provider.inner().inputs.get(), 2);
        let stats = provider.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 2);
        assert_eq!(stats.entries, 2);
    }

    #[test]
    fn cache_deduplicates_inputs_inside_one_batch() {
        let provider = CachingEmbeddingProvider::new(CountingProvider::new(8));
        let mut budget = Budget::default();
        let response = provider
            .embed(&request("m", &["same", "same", "other"]), &mut budget)
            .unwrap();
        assert_eq!(response.vectors.len(), 3);
        assert_eq!(provider.inner().inputs.get(), 2);
        assert_eq!(provider.stats().misses, 2);
        assert_eq!(response.vectors[0], response.vectors[1]);
    }

    #[test]
    fn cache_separates_models_and_dimensions() {
        let provider = CachingEmbeddingProvider::new(CountingProvider::new(8));
        let mut budget = Budget::default();
        provider
            .embed(&request("one", &["same"]), &mut budget)
            .unwrap();
        provider
            .embed(&request("two", &["same"]), &mut budget)
            .unwrap();
        assert_eq!(provider.stats().misses, 2);
        assert_eq!(provider.stats().entries, 2);
    }

    #[test]
    fn cache_evicts_entries_in_fifo_order() {
        let provider = CachingEmbeddingProvider::with_limits(CountingProvider::new(4), 1, 4096);
        let mut budget = Budget::default();
        provider.embed(&request("m", &["a"]), &mut budget).unwrap();
        provider.embed(&request("m", &["b"]), &mut budget).unwrap();
        provider.embed(&request("m", &["a"]), &mut budget).unwrap();
        let stats = provider.stats();
        assert_eq!(stats.misses, 3);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.evictions, 2);
        assert_eq!(stats.entries, 1);
    }

    #[test]
    fn cache_skips_entries_larger_than_the_byte_limit() {
        let provider = CachingEmbeddingProvider::with_limits(CountingProvider::new(64), 16, 1);
        let mut budget = Budget::default();
        let response = provider
            .embed(&request("m", &["alpha"]), &mut budget)
            .unwrap();
        assert_eq!(response.vectors.len(), 1);
        assert_eq!(provider.stats().entries, 0);
    }

    #[test]
    fn cache_clear_resets_contents_and_counters() {
        let provider = CachingEmbeddingProvider::new(CountingProvider::new(4));
        let mut budget = Budget::default();
        provider.embed(&request("m", &["a"]), &mut budget).unwrap();
        provider.clear();
        let stats = provider.stats();
        assert_eq!(stats, EmbeddingCacheStats::default());
        provider.embed(&request("m", &["a"]), &mut budget).unwrap();
        assert_eq!(provider.stats().misses, 1);
    }

    #[test]
    fn cache_rejects_providers_without_embeddings() {
        struct NoEmbeddings;
        impl Provider for NoEmbeddings {
            fn id(&self) -> &str {
                "none"
            }
            fn capabilities(&self) -> ProviderCapabilities {
                ProviderCapabilities::default()
            }
        }
        let provider = CachingEmbeddingProvider::new(NoEmbeddings);
        let mut budget = Budget::default();
        let error = provider
            .embed(&request("m", &["a"]), &mut budget)
            .unwrap_err();
        assert_eq!(error.code, super::super::provider::AI_E_UNSUPPORTED);
    }

    #[test]
    fn response_serializes_vectors_and_usage() {
        let response = EmbeddingResponse {
            model: "demo".into(),
            vectors: vec![vector(&[1.0, 0.0])],
            usage: EmbeddingUsage::new(3),
        };
        let json = response.to_json();
        assert_eq!(json.get("model").unwrap().as_str(), Some("demo"));
        assert_eq!(json.get("dimensions").unwrap().as_u64(), Some(2));
        assert_eq!(
            json.get("vectors")
                .unwrap()
                .as_array()
                .unwrap()
                .first()
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            json.get("usage")
                .unwrap()
                .get("total_tokens")
                .unwrap()
                .as_u64(),
            Some(3)
        );
    }
}
