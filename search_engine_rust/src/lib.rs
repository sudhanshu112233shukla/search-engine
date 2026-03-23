mod ingestion;
mod processing;
mod bm25;
mod embedding;
mod vector;
mod retrieval;
mod ranking;
mod extraction;
mod confidence;
mod utils;
mod ffi;
mod evaluation;

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;

pub use ingestion::{Document, load_text_dir, load_text_file};
pub use processing::ChunkingConfig;
pub use evaluation::{EvalDataset, EvalQuery, EvalReport};

use processing::{process_documents, Chunk};
use bm25::BM25Index;
use vector::VectorIndex;
use retrieval::retrieve;
use ranking::{rank_candidates, RankingWeights, Ranked};
use extraction::extract_answers;
use confidence::compute_confidence;
use utils::{make_snippet, normalize_text, process_query, tokenize};

#[derive(Clone, Debug, Serialize)]
pub struct SearchResponse {
    pub answer: Option<Answer>,
    pub answers: Vec<Answer>,
    pub results: Vec<ResultItem>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Answer {
    pub text: String,
    pub confidence: f32,
    pub source: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResultItem {
    pub id: String,
    pub text: String,
    pub score: f32,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub chunking: ChunkingConfig,
    pub bm25_k1: f32,
    pub bm25_b: f32,
    pub vector_dims: usize,
    pub vector_ngram_min: usize,
    pub vector_ngram_max: usize,
    pub retrieval_top_k: usize,
    pub results_top_k: usize,
    pub ranking_weights: RankingWeights,
    pub cache_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            chunking: ChunkingConfig::default(),
            bm25_k1: 1.5,
            bm25_b: 0.75,
            vector_dims: 384,
            vector_ngram_min: 3,
            vector_ngram_max: 5,
            retrieval_top_k: 50,
            results_top_k: 10,
            ranking_weights: RankingWeights::default(),
            cache_size: 100,
        }
    }
}

struct QueryCache {
    map: HashMap<String, SearchResponse>,
    order: VecDeque<String>,
    cap: usize,
}

impl QueryCache {
    fn new(cap: usize) -> Self {
        Self { map: HashMap::new(), order: VecDeque::new(), cap }
    }

    fn get(&mut self, key: &str) -> Option<SearchResponse> {
        if let Some(v) = self.map.get(key) {
            self.order.retain(|k| k != key);
            self.order.push_back(key.to_string());
            return Some(v.clone());
        }
        None
    }

    fn put(&mut self, key: String, value: SearchResponse) {
        if self.map.contains_key(&key) {
            self.order.retain(|k| k != &key);
        }
        self.map.insert(key.clone(), value);
        self.order.push_back(key);
        while self.order.len() > self.cap {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
    }
}

pub struct SearchEngine {
    chunks: Vec<Chunk>,
    bm25: BM25Index,
    vector: VectorIndex,
    config: Config,
    cache: Mutex<QueryCache>,
}

impl SearchEngine {
    pub fn new(docs: Vec<Document>, config: Config) -> Self {
        let chunks = process_documents(&docs, &config.chunking);
        let bm25 = BM25Index::build(&chunks, config.bm25_k1, config.bm25_b);
        let vector = VectorIndex::build(&chunks, config.vector_dims, config.vector_ngram_min, config.vector_ngram_max);
        let cache = Mutex::new(QueryCache::new(config.cache_size));
        Self { chunks, bm25, vector, config, cache }
    }

    pub fn search(&self, query: &str) -> SearchResponse {
        if query.trim().is_empty() {
            return SearchResponse { answer: None, answers: Vec::new(), results: Vec::new() };
        }

        let cache_key = normalize_text(query);
        if let Ok(mut cache) = self.cache.lock() {
            if let Some(cached) = cache.get(&cache_key) {
                return cached;
            }
        }

        let response = self.search_inner(query);
        if let Ok(mut cache) = self.cache.lock() {
            cache.put(cache_key, response.clone());
        }
        response
    }

    fn search_inner(&self, query: &str) -> SearchResponse {
        let base_tokens = tokenize(query);
        let expanded_tokens = process_query(query);
        let clean_query = normalize_text(query);

        let (bm25_results, vector_results) = retrieve(
            &self.bm25,
            &self.vector,
            &expanded_tokens,
            query,
            self.config.retrieval_top_k,
        );

        let ranked = rank_candidates(
            &bm25_results,
            &vector_results,
            &self.chunks,
            &base_tokens,
            &clean_query,
            &self.config.ranking_weights,
        );

        let mut results = Vec::new();
        for r in ranked.iter().take(self.config.results_top_k) {
            let chunk = &self.chunks[r.doc_id];
            let snippet = make_snippet(&chunk.text, &base_tokens, 180);
            results.push(ResultItem {
                id: chunk.id.clone(),
                text: snippet,
                score: r.score,
            });
        }

        let candidates = extract_answers(query, &ranked, &self.chunks);
        let mut answers: Vec<Answer> = candidates
            .iter()
            .take(3)
            .map(|a| {
                let (confidence, _level) = compute_confidence(a.score);
                Answer {
                    text: a.text.clone(),
                    confidence,
                    source: a.source.clone(),
                }
            })
            .collect();

        let answer = answers.first().cloned();
        if let Some(top) = answer.as_ref() {
            if top.confidence > 0.5 {
                answers.truncate(1);
            }
        }

        SearchResponse { answer, answers, results }
    }

    pub fn rank_debug(&self, query: &str) -> (Vec<Ranked>, Vec<(String, f32, crate::ranking::ScoreBreakdown)>) {
        let base_tokens = tokenize(query);
        let expanded_tokens = process_query(query);
        let clean_query = normalize_text(query);

        let (bm25_results, vector_results) = retrieve(
            &self.bm25,
            &self.vector,
            &expanded_tokens,
            query,
            self.config.retrieval_top_k,
        );

        let ranked = rank_candidates(
            &bm25_results,
            &vector_results,
            &self.chunks,
            &base_tokens,
            &clean_query,
            &self.config.ranking_weights,
        );

        let breakdowns = ranked.iter().map(|r| {
            let id = self.chunks[r.doc_id].id.clone();
            (id, r.score, r.breakdown.clone())
        }).collect();

        (ranked, breakdowns)
    }

    pub fn doc_id(&self, idx: usize) -> Option<String> {
        self.chunks.get(idx).map(|c| c.id.clone())
    }

    pub fn approx_memory_bytes(&self) -> usize {
        let mut total = 0usize;
        for c in &self.chunks {
            total += c.text.len();
            total += c.clean.len();
            for t in &c.tokens {
                total += t.len();
            }
            for positions in c.positions.values() {
                total += positions.len() * std::mem::size_of::<usize>();
            }
        }
        total += self.vector.vectors.len() * self.vector.dims * std::mem::size_of::<f32>();
        total
    }
}

static ENGINE: OnceLock<SearchEngine> = OnceLock::new();

pub fn init_engine(engine: SearchEngine) {
    let _ = ENGINE.set(engine);
}

pub fn init_engine_once(engine: SearchEngine) -> bool {
    ENGINE.set(engine).is_ok()
}

pub fn search(query: &str) -> SearchResponse {
    if let Some(engine) = ENGINE.get() {
        engine.search(query)
    } else {
        SearchResponse { answer: None, answers: Vec::new(), results: Vec::new() }
    }
}
