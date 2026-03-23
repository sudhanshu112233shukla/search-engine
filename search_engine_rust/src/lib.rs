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

use std::sync::OnceLock;

pub use ingestion::{Document, load_text_dir, load_text_file};
pub use processing::{ChunkingConfig};

use processing::{process_documents, Chunk};
use bm25::BM25Index;
use vector::VectorIndex;
use retrieval::retrieve;
use ranking::hybrid_rank;
use extraction::extract_answer;
use confidence::compute_confidence;

#[derive(Clone, Debug)]
pub struct SearchResponse {
    pub answer: Option<Answer>,
    pub results: Vec<ResultItem>,
}

#[derive(Clone, Debug)]
pub struct Answer {
    pub text: String,
    pub confidence: f32,
    pub source: String,
}

#[derive(Clone, Debug)]
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
        }
    }
}

pub struct SearchEngine {
    chunks: Vec<Chunk>,
    bm25: BM25Index,
    vector: VectorIndex,
    config: Config,
}

impl SearchEngine {
    pub fn new(docs: Vec<Document>, config: Config) -> Self {
        let chunks = process_documents(&docs, &config.chunking);
        let bm25 = BM25Index::build(&chunks, config.bm25_k1, config.bm25_b);
        let vector = VectorIndex::build(&chunks, config.vector_dims, config.vector_ngram_min, config.vector_ngram_max);
        Self { chunks, bm25, vector, config }
    }

    pub fn search(&self, query: &str) -> SearchResponse {
        if query.trim().is_empty() {
            return SearchResponse { answer: None, results: Vec::new() };
        }

        let candidates = retrieve(&self.bm25, &self.vector, query, self.config.retrieval_top_k);
        let ranked = hybrid_rank(candidates);

        let mut results = Vec::new();
        for r in ranked.iter().take(self.config.results_top_k) {
            let chunk = &self.chunks[r.doc_id];
            results.push(ResultItem {
                id: chunk.id.clone(),
                text: chunk.text.clone(),
                score: r.score,
            });
        }

        let answer = extract_answer(query, &ranked, &self.chunks).map(|a| {
            let (confidence, _level) = compute_confidence(a.score);
            Answer {
                text: a.text,
                confidence,
                source: a.source,
            }
        });

        SearchResponse { answer, results }
    }
}

static ENGINE: OnceLock<SearchEngine> = OnceLock::new();

pub fn init_engine(engine: SearchEngine) {
    let _ = ENGINE.set(engine);
}

pub fn search(query: &str) -> SearchResponse {
    if let Some(engine) = ENGINE.get() {
        engine.search(query)
    } else {
        SearchResponse { answer: None, results: Vec::new() }
    }
}
